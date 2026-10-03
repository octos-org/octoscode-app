//! Transport-agnostic UI Protocol core shared by the WebSocket and stdio
//! transports. Everything here operates on JSON-RPC *text frames* and the
//! `OutboundCommand` / `TransportEvent` channel types — it never touches the
//! byte transport itself. The `ws` and `stdio` modules own the socket / pipe
//! and delegate command dispatch (`build_outbound`) and inbound handling
//! (`handle_inbound_text`) here so there is a single source of truth for the
//! wire contract regardless of how frames travel.

use std::collections::HashMap;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::{
    methods, ApprovalRespondResult, DiffPreviewGetResult, RpcError, TaskOutputReadResult, UiCursor,
    UiRpcResult,
};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use octos_core::SessionKey;

use crate::capability::Capabilities;
use crate::cursor::{CursorPersist, CursorStore};
use crate::jsonrpc::{serialize_request, JsonRpcId, RpcEnvelope, RpcRegistry};
use crate::{ConnectionState, LifecycleResult, OutboundCommand, TransportEvent};

/// Bounded channel depth for both the outbound command queue and the inbound
/// event queue (shared by every transport).
pub const CHANNEL_BUFFER: usize = 64;

pub(crate) struct PendingRequest {
    /// The method name, owned: generic `Request` carries a runtime string.
    pub(crate) method: String,
    pub(crate) reply: PendingReply,
}

pub(crate) enum PendingReply {
    Lifecycle,
    Approval(oneshot::Sender<Result<ApprovalRespondResult, RpcError>>),
    DiffPreview(oneshot::Sender<Result<DiffPreviewGetResult, RpcError>>),
    TaskOutput(oneshot::Sender<Result<TaskOutputReadResult, RpcError>>),
    /// `session/list` — result re-emitted as `TransportEvent::SessionsListed`.
    SessionList,
    /// `session/hydrate` — result re-emitted as
    /// `TransportEvent::SessionHydrated` tagged with the session key.
    SessionHydrate { session_id: String },
    /// A generic JSON-RPC request (`OutboundCommand::Request`): the raw
    /// `result` value or the server's `RpcError` goes straight back on the
    /// caller's oneshot. No event is emitted.
    Generic(oneshot::Sender<Result<Value, RpcError>>),
    /// A `session/open` the transport re-sent itself after the shell's kernel
    /// restarted (see `kernel`): it brings the connection back to `Live`,
    /// and the app, which already holds the session, hears nothing else.
    Reopen,
}

/// Per-connection mutable state: the replay cursor, in-flight requests keyed
/// by JSON-RPC id, and the id registry.
pub(crate) struct SharedState {
    /// Per-session replay cursors (W08 multi-session). Keyed by `SessionKey`
    /// so concurrent live sessions never clobber each other's replay position.
    pub(crate) cursors: CursorStore,
    /// One-shot legacy seed: the single `TransportConfig.cursor` (usually None),
    /// applied to the first bracketed `session/open` that has no stored cursor.
    pub(crate) pending_initial: Option<UiCursor>,
    pub(crate) pending: HashMap<JsonRpcId, PendingRequest>,
    pub(crate) registry: std::sync::Arc<RpcRegistry>,
    /// The sessions opened on this connection, as last opened: the kernel
    /// transport opens them again (from their cursors) on a new kernel.
    pub(crate) opened: HashMap<SessionKey, octos_core::app_ui::AppUiOpenSession>,
}

impl SharedState {
    pub(crate) fn new(
        cursor: Option<UiCursor>,
        persist: Option<std::sync::Arc<dyn CursorPersist>>,
    ) -> Self {
        Self {
            cursors: match persist {
                Some(p) => CursorStore::new_persisted(p),
                None => CursorStore::new(),
            },
            pending_initial: cursor,
            pending: HashMap::new(),
            registry: std::sync::Arc::new(RpcRegistry::new()),
            opened: HashMap::new(),
        }
    }
}

/// Result of translating an `OutboundCommand` into a wire frame. The transport
/// sends `frame` its own way (WS text message / stdin line) and, on a
/// successful send, records `pending` under `id`.
pub(crate) enum Outbound {
    Send {
        id: JsonRpcId,
        frame: String,
        pending: Option<PendingRequest>,
    },
    /// Serialization failed — skip this command (already logged).
    Skip,
    /// `OutboundCommand::Disconnect` — the transport should drain and exit.
    Disconnect,
}

fn to_value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Translate an `OutboundCommand` into a serialized JSON-RPC request frame plus
/// the `PendingReply` to record when it is sent. Consumes a fresh id from the
/// registry. Transport-agnostic: the caller owns the actual write.
pub(crate) fn build_outbound(cmd: OutboundCommand, shared: &mut SharedState) -> Outbound {
    // A generic JSON-RPC request carries a runtime method name and wants the
    // raw `result`. It rides the SAME `serialize_request` + `RpcRegistry` path
    // as the typed commands below; only the method string is dynamic.
    let cmd = match cmd {
        OutboundCommand::Request { method, params, reply } => {
            let id = shared.registry.next_id();
            return match serialize_request(&id, &method, &params) {
                Ok(frame) => Outbound::Send {
                    id,
                    frame,
                    pending: Some(PendingRequest {
                        method,
                        reply: PendingReply::Generic(reply),
                    }),
                },
                Err(e) => {
                    log::warn!("transport: serialize {method}: {e}");
                    Outbound::Skip
                }
            };
        }
        other => other,
    };
    let id = shared.registry.next_id();
    let (method, body, pending): (&'static str, Value, Option<PendingReply>) = match cmd {
        OutboundCommand::OpenSession(mut params) => {
            // Resume bracket: replay from THIS session's last cursor (W08
            // multi-session), falling back to the one-shot legacy seed for the
            // very first open. Per-session so concurrent sessions never share a
            // cursor.
            if params.after.is_none() {
                params.after = shared
                    .cursors
                    .get(&params.session_id)
                    .cloned()
                    .or_else(|| shared.pending_initial.take());
            }
            shared.opened.insert(params.session_id.clone(), params.clone());
            (methods::SESSION_OPEN, to_value(&params), Some(PendingReply::Lifecycle))
        }
        OutboundCommand::OpenSessionFresh(params) => {
            // Open WITHOUT a replay bracket (`params.after` stays None). With
            // per-session cursors (W08) there is no shared cursor to reset —
            // every other session keeps its own replay position.
            shared.opened.insert(params.session_id.clone(), params.clone());
            (methods::SESSION_OPEN, to_value(&params), Some(PendingReply::Lifecycle))
        }
        OutboundCommand::StartTurn(p) => {
            (methods::TURN_START, to_value(&p), Some(PendingReply::Lifecycle))
        }
        OutboundCommand::InterruptTurn(p) => {
            (methods::TURN_INTERRUPT, to_value(&p), Some(PendingReply::Lifecycle))
        }
        OutboundCommand::SendApprovalResponse { params, reply } => {
            (methods::APPROVAL_RESPOND, to_value(&params), Some(PendingReply::Approval(reply)))
        }
        OutboundCommand::FetchDiffPreview { params, reply } => {
            (methods::DIFF_PREVIEW_GET, to_value(&params), Some(PendingReply::DiffPreview(reply)))
        }
        OutboundCommand::RequestTaskOutput { params, reply } => {
            (methods::TASK_OUTPUT_READ, to_value(&params), Some(PendingReply::TaskOutput(reply)))
        }
        OutboundCommand::ListSessions => (
            methods::SESSION_LIST,
            // `cwd: None` = legacy per-profile listing; the field is
            // skip_serializing_if so the wire shape stays the historical
            // empty object.
            to_value(&octos_core::ui_protocol::SessionListParams { cwd: None, ..Default::default() }),
            Some(PendingReply::SessionList),
        ),
        OutboundCommand::HydrateSession { session_id } => (
            methods::SESSION_HYDRATE,
            to_value(&octos_core::ui_protocol::SessionHydrateParams {
                session_id: octos_core::SessionKey(session_id.clone()),
                after: None,
                include: vec!["messages".to_owned()],
            }),
            Some(PendingReply::SessionHydrate { session_id }),
        ),
        OutboundCommand::Disconnect => return Outbound::Disconnect,
        // Handled by the early return above; unreachable here.
        OutboundCommand::Request { .. } => unreachable!("Request is routed before this match"),
    };

    match serialize_request(&id, method, &body) {
        Ok(frame) => Outbound::Send {
            id,
            frame,
            pending: pending.map(|reply| PendingRequest { method: method.to_string(), reply }),
        },
        Err(e) => {
            log::warn!("transport: serialize {method}: {e}");
            Outbound::Skip
        }
    }
}

/// `session/open` frames for every session opened on this connection, each
/// from its own replay cursor, for a new kernel after a restart. The caller
/// sends them and records each `pending` under its `id`.
pub(crate) fn build_reopens(shared: &mut SharedState) -> Vec<(JsonRpcId, String, PendingRequest)> {
    let opened: Vec<_> = shared.opened.values().cloned().collect();
    let mut out = Vec::new();
    for mut params in opened {
        params.after = shared.cursors.get(&params.session_id).cloned();
        let id = shared.registry.next_id();
        match serialize_request(&id, methods::SESSION_OPEN, &to_value(&params)) {
            Ok(frame) => out.push((id, frame, PendingRequest { method: methods::SESSION_OPEN.to_string(), reply: PendingReply::Reopen })),
            Err(e) => log::warn!("transport: serialize session/open (reopen): {e}"),
        }
    }
    out
}

/// Fail every request still waiting for a reply: the kernel that would have
/// answered is gone. Lifecycle requests surface as `TransportEvent::RpcError`
/// (the app shows a turn that could not start), the rest through their reply.
pub(crate) async fn fail_all_pending(shared: &mut SharedState, events: &mpsc::Sender<TransportEvent>, error: RpcError) {
    let pending: Vec<_> = shared.pending.drain().collect();
    for (id, pending) in pending {
        let method = pending.method.to_owned();
        let surface = matches!(pending.reply, PendingReply::Lifecycle);
        fail_pending(pending, error.clone());
        if surface {
            emit(events, TransportEvent::RpcError { request_id: id, method, error: error.clone() }).await;
        }
    }
}

/// `message/delta` is the only ephemeral notification per
/// `03-PROTOCOL-CONTRACT.md` § "Live streaming output".
pub(crate) fn is_ephemeral_method(method: &str) -> bool {
    method == methods::MESSAGE_DELTA
}

/// Hand `evt` to the consumer, WAITING for room in the bounded event channel.
///
/// Nothing is dropped: not a reply, an error reply, a notification or a
/// connection-state change. A reply in particular may be the only answer to a
/// request the consumer is counting on — a history read, a `session/open` —
/// and a consumer that never hears it stays one reply behind for good. When
/// the consumer is slow, the read loop waits here and TCP flow control
/// reaches the server, which is how notifications were already delivered.
/// Only a consumer that is gone (the receiver dropped) ends delivery, and
/// then there is nobody left to tell.
pub(crate) async fn emit(events: &mpsc::Sender<TransportEvent>, evt: TransportEvent) {
    if let Err(mpsc::error::SendError(evt)) = events.send(evt).await {
        log::debug!("transport: event receiver closed; {} not delivered", event_kind(&evt));
    }
}

/// A short name for an event (logs; a reply's payload can be large).
fn event_kind(evt: &TransportEvent) -> &'static str {
    match evt {
        TransportEvent::ConnectionState(_) => "a connection state",
        TransportEvent::DurableNotification { .. } => "a notification",
        TransportEvent::EphemeralNotification { .. } => "a live delta",
        TransportEvent::RpcResult(_) => "a reply",
        TransportEvent::RpcError { .. } => "an error reply",
        TransportEvent::CapabilityNegotiated(_) => "the capabilities",
        TransportEvent::SessionsListed { .. } => "a session/list reply",
        TransportEvent::SessionHydrated { .. } => "a session/hydrate reply",
    }
}

/// Inbound text-frame dispatcher. Returns `Some(new_state)` if the frame
/// implies a `ConnectionState` transition the caller should announce.
pub(crate) async fn handle_inbound_text(
    text: &str,
    shared: &mut SharedState,
    events: &mpsc::Sender<TransportEvent>,
    state: &mut ConnectionState,
) -> Option<ConnectionState> {
    let env = match RpcEnvelope::parse(text) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("transport: bad json frame: {e}");
            return None;
        }
    };
    match env {
        RpcEnvelope::Notification(n) => {
            handle_notification(&n.method, n.params, shared, events).await;
            None
        }
        RpcEnvelope::Response(r) => match shared.pending.remove(&r.id) {
            Some(p) => handle_response(p, r.result, events, state).await,
            None => {
                log::warn!("transport: response for unknown id {}", r.id);
                None
            }
        },
        RpcEnvelope::ErrorResponse(er) => {
            if let Some(id) = er.id.clone() {
                if let Some(pending) = shared.pending.remove(&id) {
                    let method = pending.method.to_owned();
                    let reopen = matches!(pending.reply, PendingReply::Reopen);
                    fail_pending(pending, er.error.clone());
                    if !reopen {
                        emit(
                            events,
                            TransportEvent::RpcError {
                                request_id: id,
                                method,
                                error: er.error,
                            },
                        )
                        .await;
                    }
                }
            } else {
                log::warn!("transport: error response missing id: {:?}", er.error);
            }
            None
        }
        RpcEnvelope::Request(req) => {
            log::warn!("transport: server initiated request {} (ignored)", req.method);
            None
        }
    }
}

async fn handle_response(
    pending: PendingRequest,
    result_value: Value,
    events: &mpsc::Sender<TransportEvent>,
    state: &mut ConnectionState,
) -> Option<ConnectionState> {
    let method = pending.method;
    match pending.reply {
        PendingReply::Reopen => {
            if !matches!(state, ConnectionState::Live) {
                *state = ConnectionState::Live;
                return Some(ConnectionState::Live);
            }
            None
        }
        PendingReply::Lifecycle => {
            match UiRpcResult::from_method_and_result(&method, result_value.clone()) {
                Ok(UiRpcResult::SessionOpen(open)) => {
                    let caps = Capabilities::parse(&result_value);
                    emit(events, TransportEvent::CapabilityNegotiated(caps)).await;
                    emit(events, TransportEvent::RpcResult(LifecycleResult::SessionOpen(open))).await;
                    if !matches!(state, ConnectionState::Live) {
                        *state = ConnectionState::Live;
                        return Some(ConnectionState::Live);
                    }
                    None
                }
                Ok(UiRpcResult::TurnStart(r)) => {
                    emit(events, TransportEvent::RpcResult(LifecycleResult::TurnStart(r))).await;
                    None
                }
                Ok(UiRpcResult::TurnInterrupt(r)) => {
                    emit(events, TransportEvent::RpcResult(LifecycleResult::TurnInterrupt(r))).await;
                    None
                }
                Ok(other) => {
                    log::warn!("transport: lifecycle result unexpected variant: {:?}", other.kind());
                    None
                }
                Err(e) => {
                    log::warn!("transport: decode lifecycle result for {method}: {e:?}");
                    None
                }
            }
        }
        PendingReply::Approval(reply) => {
            let _ = reply.send(
                serde_json::from_value::<ApprovalRespondResult>(result_value)
                    .map_err(|e| RpcError::invalid_params(e.to_string())),
            );
            None
        }
        PendingReply::DiffPreview(reply) => {
            let _ = reply.send(
                serde_json::from_value::<DiffPreviewGetResult>(result_value)
                    .map_err(|e| RpcError::invalid_params(e.to_string())),
            );
            None
        }
        PendingReply::TaskOutput(reply) => {
            let _ = reply.send(
                serde_json::from_value::<TaskOutputReadResult>(result_value)
                    .map_err(|e| RpcError::invalid_params(e.to_string())),
            );
            None
        }
        PendingReply::SessionList => {
            match serde_json::from_value::<octos_core::ui_protocol::SessionListResult>(result_value)
            {
                Ok(r) => emit(events, TransportEvent::SessionsListed { sessions: r.sessions }).await,
                Err(e) => log::warn!("transport: decode session/list result: {e}"),
            }
            None
        }
        PendingReply::SessionHydrate { session_id } => {
            // Raw pass-through: the backend decodes `SessionHydrateResult`
            // (it owns the chat-store routing; keeps the transport thin).
            emit(events, TransportEvent::SessionHydrated { session_id, result: result_value }).await;
            None
        }
        PendingReply::Generic(reply) => {
            // Raw pass-through: the caller decodes the method's result type.
            let _ = reply.send(Ok(result_value));
            None
        }
    }
}

fn fail_pending(pending: PendingRequest, err: RpcError) {
    match pending.reply {
        PendingReply::Lifecycle => {} // surfaced as TransportEvent::RpcError
        PendingReply::Approval(reply) => {
            let _ = reply.send(Err(err));
        }
        PendingReply::DiffPreview(reply) => {
            let _ = reply.send(Err(err));
        }
        PendingReply::TaskOutput(reply) => {
            let _ = reply.send(Err(err));
        }
        // Sidebar hydrate is best-effort; the retry rides the next
        // `session/open` → `CapabilityNegotiated` → `ListSessions` cycle.
        PendingReply::SessionList => {}
        // History hydrate is best-effort too — the user can re-tap the
        // session row; the error already surfaced as a warn.
        PendingReply::SessionHydrate { session_id } => {
            log::warn!("transport: session/hydrate failed for {session_id}: {err:?}");
        }
        PendingReply::Reopen => {
            log::warn!("transport: re-opening a session on the restarted kernel failed: {err:?}");
        }
        PendingReply::Generic(reply) => {
            let _ = reply.send(Err(err));
        }
    }
}

async fn handle_notification(
    method: &str,
    params: Value,
    shared: &mut SharedState,
    events: &mpsc::Sender<TransportEvent>,
) {
    let payload = match UiNotification::from_method_and_params(method, params.clone()) {
        Ok(p) => p,
        Err(_) => {
            // `server/heartbeat` is a periodic keepalive (~20s, empty params)
            // with no app-facing payload — ignore it quietly rather than
            // logging an "unknown notification" warning on every tick.
            if method != "server/heartbeat" {
                log::warn!("transport: unknown notification method '{method}'");
            }
            return;
        }
    };
    if is_ephemeral_method(method) {
        // These are text deltas, not replaceable status snapshots. A fast
        // cached response can outpace mobile rendering. Apply backpressure
        // instead of dropping bytes and corrupting the generated card.
        emit(events, TransportEvent::EphemeralNotification { payload }).await;
        return;
    }
    let cursor = params
        .get("cursor")
        .and_then(|v| serde_json::from_value::<UiCursor>(v.clone()).ok());
    if let Some(c) = cursor.clone() {
        // W08: advance the cursor for THIS notification's session only, so
        // concurrent live sessions don't overwrite each other's replay position.
        if let Some(session) = params
            .get("session_id")
            .and_then(|v| serde_json::from_value::<SessionKey>(v.clone()).ok())
        {
            shared.cursors.set(session, c);
        }
    }
    emit(events, TransportEvent::DurableNotification { payload, cursor }).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_helper_only_message_delta() {
        assert!(is_ephemeral_method(methods::MESSAGE_DELTA));
        assert!(!is_ephemeral_method(methods::TOOL_STARTED));
    }

    #[tokio::test]
    async fn slow_consumer_receives_every_text_fragment_in_order() {
        let (tx, mut rx) = mpsc::channel(1);
        let producer = tokio::spawn(async move {
            let mut shared = SharedState::new(None, None);
            for text in ["<card>", "東京", "</card>"] {
                handle_notification(methods::MESSAGE_DELTA, serde_json::json!({
                    "session_id": "_main:test", "turn_id": "00000000-0000-7000-8000-000000000001",
                    "text": text
                }), &mut shared, &tx).await;
            }
        });
        // Deliberately let the producer fill the single-slot queue.
        tokio::task::yield_now().await;
        let mut text = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                TransportEvent::EphemeralNotification { payload: UiNotification::MessageDelta(delta) } => text.push_str(&delta.text),
                other => panic!("unexpected event: {other:?}"),
            }
            tokio::task::yield_now().await;
        }
        producer.await.unwrap();
        assert_eq!(text, "<card>東京</card>");
    }

    /// A generic `Request` rides the same serialize path as the typed
    /// commands: the frame is well-formed JSON-RPC 2.0 with the caller's
    /// method + params, and its reply oneshot is recorded under the frame's id.
    #[tokio::test]
    async fn generic_request_builds_a_jsonrpc_frame_and_registers_its_reply() {
        let mut shared = SharedState::new(None, None);
        let (tx, mut rx) = oneshot::channel::<Result<Value, RpcError>>();
        let out = build_outbound(
            OutboundCommand::Request {
                method: "tool/status/list".to_owned(),
                params: serde_json::json!({"session_id": "_main:test", "profile_id": "p"}),
                reply: tx,
            },
            &mut shared,
        );
        let Outbound::Send { id, frame, pending } = out else {
            panic!("expected Outbound::Send");
        };
        let parsed: Value = serde_json::from_str(&frame).expect("frame is JSON");
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], id.as_str());
        assert_eq!(parsed["method"], "tool/status/list");
        assert_eq!(parsed["params"]["session_id"], "_main:test");
        let pending = pending.expect("a generic request records a pending reply");
        assert_eq!(pending.method, "tool/status/list");

        // Delivery: a server result reaches the caller's oneshot verbatim.
        let value = serde_json::json!({"tools": []});
        let _ = handle_response(pending, value.clone(), &mpsc::channel(1).0, &mut ConnectionState::Live).await;
        assert_eq!(rx.try_recv().expect("reply sent").expect("ok"), value);
    }

    /// Record `cmd` as sent (as the ws / kernel loops do) and return its id.
    fn sent(cmd: OutboundCommand, shared: &mut SharedState) -> String {
        let Outbound::Send { id, pending, .. } = build_outbound(cmd, shared) else {
            panic!("expected Outbound::Send");
        };
        shared.pending.insert(id.clone(), pending.expect("a reply is awaited"));
        id.as_str().to_owned()
    }

    fn hydrate_reply(id: &str, session: &str, seq: u64) -> String {
        serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {
            "session_id": session, "cursor": {"stream": session, "seq": seq}, "messages": []
        }})
        .to_string()
    }

    fn envelope(session: &str, seq: u64) -> String {
        serde_json::json!({"jsonrpc": "2.0", "method": "projection/envelope", "params": {
            "session_id": session, "thread_id": "01920000-0000-7000-8000-000000000001",
            "turn_id": "01920000-0000-7000-8000-000000000001", "seq": seq,
            "cursor": {"stream": session, "seq": seq},
            "payload": {"type": "assistant_delta", "data": {"text": "x", "assistant_segment_id": "s"}}
        }})
        .to_string()
    }

    /// What the consumer saw, in order: `reply <session>` for a history
    /// reply, `error <method>` for an error reply, `listed` / `turn` for the
    /// typed replies, `n<seq>` for a notification.
    fn label(evt: &TransportEvent) -> String {
        match evt {
            TransportEvent::SessionHydrated { session_id, .. } => format!("reply {session_id}"),
            TransportEvent::RpcError { method, .. } => format!("error {method}"),
            TransportEvent::SessionsListed { .. } => "listed".to_owned(),
            TransportEvent::RpcResult(LifecycleResult::TurnStart(_)) => "turn".to_owned(),
            TransportEvent::DurableNotification { cursor, .. } => {
                format!("n{}", cursor.as_ref().map(|c| c.seq).unwrap_or_default())
            }
            other => format!("{other:?}"),
        }
    }

    /// A server reply that finds the event channel full WAITS for room — it
    /// is never dropped. (Before: `try_emit` dropped it with a warning, and a
    /// consumer counting its history reads stayed one read behind for good.)
    #[tokio::test]
    async fn a_reply_that_finds_the_event_channel_full_waits_and_is_never_dropped() {
        let (tx, mut rx) = mpsc::channel(1);
        // The consumer has not taken the last event yet: the channel is full.
        tx.send(TransportEvent::ConnectionState(ConnectionState::Live)).await.unwrap();
        let mut shared = SharedState::new(None, None);
        let id = sent(OutboundCommand::HydrateSession { session_id: "p:api:b".into() }, &mut shared);
        let frame = hydrate_reply(&id, "p:api:b", 3);
        let reader = tokio::spawn(async move {
            let mut state = ConnectionState::Live;
            handle_inbound_text(&frame, &mut shared, &tx, &mut state).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut seen = Vec::new();
        while let Some(evt) = rx.recv().await {
            seen.push(label(&evt));
        }
        reader.await.unwrap();
        assert_eq!(seen, vec!["ConnectionState(Live)".to_owned(), "reply p:api:b".to_owned()], "the reply was dropped");
    }

    /// Replies of every kind — history reads for three Sessions, an error
    /// reply, a typed `session/list` and `turn/start` result — arrive through
    /// a one-slot channel and a slow consumer interleaved with notifications,
    /// all of them, in the order the server sent them.
    #[tokio::test]
    async fn every_reply_arrives_in_order_through_a_full_channel() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut shared = SharedState::new(None, None);
        let h1 = sent(OutboundCommand::HydrateSession { session_id: "p:api:a".into() }, &mut shared);
        let h2 = sent(OutboundCommand::HydrateSession { session_id: "p:api:b".into() }, &mut shared);
        let h3 = sent(OutboundCommand::HydrateSession { session_id: "p:api:c".into() }, &mut shared);
        let h4 = sent(OutboundCommand::HydrateSession { session_id: "p:api:d".into() }, &mut shared);
        let ls = sent(OutboundCommand::ListSessions, &mut shared);
        let ts = sent(
            OutboundCommand::StartTurn(serde_json::from_value(serde_json::json!({
                "session_id": "p:api:a", "turn_id": "01920000-0000-7000-8000-000000000002",
                "input": [{"kind": "text", "text": "go"}]
            })).expect("turn/start params")),
            &mut shared,
        );
        let mut frames = Vec::new();
        let mut want = Vec::new();
        for n in 1..=5 {
            frames.push(envelope("p:api:a", n));
            want.push(format!("n{n}"));
        }
        frames.push(hydrate_reply(&h1, "p:api:a", 5));
        want.push("reply p:api:a".to_owned());
        for n in 6..=8 {
            frames.push(envelope("p:api:a", n));
            want.push(format!("n{n}"));
        }
        frames.push(hydrate_reply(&h2, "p:api:b", 1));
        want.push("reply p:api:b".to_owned());
        frames.push(serde_json::json!({"jsonrpc": "2.0", "id": h3, "error": {"code": -32603, "message": "busy"}}).to_string());
        want.push("error session/hydrate".to_owned());
        frames.push(serde_json::json!({"jsonrpc": "2.0", "id": ls, "result": {"sessions": []}}).to_string());
        want.push("listed".to_owned());
        frames.push(envelope("p:api:a", 9));
        want.push("n9".to_owned());
        frames.push(serde_json::json!({"jsonrpc": "2.0", "id": ts, "result": {"accepted": true}}).to_string());
        want.push("turn".to_owned());
        frames.push(hydrate_reply(&h4, "p:api:d", 2));
        want.push("reply p:api:d".to_owned());
        let reader = tokio::spawn(async move {
            let mut state = ConnectionState::Live;
            for f in frames {
                handle_inbound_text(&f, &mut shared, &tx, &mut state).await;
            }
        });
        let mut seen = Vec::new();
        while let Some(evt) = rx.recv().await {
            seen.push(label(&evt));
            // A slow consumer (a busy UI thread).
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        reader.await.unwrap();
        assert_eq!(seen, want, "every reply, in the server's order");
    }

    /// An RPC error on a generic request reaches the caller's oneshot as an
    /// `Err`, not as an event.
    #[test]
    fn generic_request_error_reaches_the_caller() {
        let mut shared = SharedState::new(None, None);
        let (tx, mut rx) = oneshot::channel::<Result<Value, RpcError>>();
        let out = build_outbound(
            OutboundCommand::Request {
                method: "does/not/exist".to_owned(),
                params: Value::Null,
                reply: tx,
            },
            &mut shared,
        );
        let Outbound::Send { pending, .. } = out else {
            panic!("expected Outbound::Send");
        };
        let err = RpcError { code: -32601, message: "method not found".to_owned(), data: None };
        fail_pending(pending.expect("pending"), err.clone());
        let got = rx.try_recv().expect("reply sent");
        assert_eq!(got.expect_err("an error reply").message, "method not found");
    }
}
