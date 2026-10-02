//! `approval/*` — the approval sheet (request, decide, cancel).
//!
//! Requests implemented here: `approval/scopes/list`, `user_question/respond`.
//! `approval/respond` stays on the transport's typed `OutboundCommand` (it
//! carries a oneshot reply and is gated by the approval feature). Notifications
//! this file handles: `approval/requested` (stores the pending row),
//! `approval/decided`, `approval/cancelled` and `approval/auto_resolved`
//! (settle the pending row so the sheet reflects the decision).
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::approval::PendingQuestion;
use octoscode_store::Store;

use crate::method::Method;
use crate::registry::{NotificationHandler, Registry};

/// `approval/requested` — the server is asking the person to decide.
pub struct ApprovalRequestedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ApprovalRequestedHandler {
    const METHOD: &'static str = methods::APPROVAL_REQUESTED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ApprovalRequested(requested) = notification {
            self.store.note_seen(Self::METHOD);
            // Keep the pending approval so the sheet has something to render
            // and `approval/respond` has an id to answer.
            //
            // #P4f2 row 7: also keep the diff preview id the PAYLOAD carries
            // (`typedDetails.diff.preview_id`, web `approvalDiffPreviewId` —
            // `packages/client/src/interaction.ts:94-102`), validated through
            // the shared protocol-id gate so a non-id string cannot bind `D`.
            // The web reads only this one contract location and never scrapes
            // prose ("it never recursively scrapes prose", :104).
            let preview_id = requested
                .typed_details
                .as_ref()
                .and_then(|d| d.diff.as_ref())
                .map(|d| crate::protocol_id::preview_id_string(&d.preview_id))
                .filter(|id| crate::protocol_id::is_protocol_uuid(&serde_json::json!(id)));
            let id = requested.approval_id.0.to_string();
            self.store.domains.approval.request_with_preview(
                &id,
                Some(requested.tool_name.clone()),
                preview_id,
            );
            // A6: the takeover card's payload (`ApprovalPanel.tsx:58-87`:
            // title, body, risk, tool, kind, the typed command), scoped to
            // the session + turn the request names.
            self.store.domains.approval.set_detail(&id, approval_detail(requested));
        }
    }
}

/// A6 — the card payload of one `approval/requested` (the web's
/// `parseApprovalRequested`, `interaction.ts:41-79`, plus `approvalCommand`,
/// `ApprovalPanel.tsx:131-139`: `typed_details.command.command_line`, else
/// its `argv` joined by spaces when every element is a string).
pub fn approval_detail(
    requested: &octos_core::ui_protocol::ApprovalRequestedEvent,
) -> octoscode_store::domains::approval::ApprovalDetail {
    let command = requested
        .typed_details
        .as_ref()
        .and_then(|d| serde_json::to_value(d).ok())
        .and_then(|v| command_of(&v));
    octoscode_store::domains::approval::ApprovalDetail {
        session_id: requested.session_id.0.clone(),
        turn_id: requested.turn_id.0.to_string(),
        tool_name: requested.tool_name.clone(),
        title: requested.title.clone(),
        body: requested.body.clone(),
        kind: requested.approval_kind.clone(),
        risk: requested.risk.clone(),
        command,
    }
}

/// `approvalCommand` (`ApprovalPanel.tsx:131-139`) over the wire JSON of
/// `typed_details`: `command.command_line` if it is a string, else
/// `command.argv` joined by spaces if every element is a string, else none.
pub fn command_of(typed_details: &serde_json::Value) -> Option<String> {
    let command = typed_details.get("command")?.as_object()?;
    if let Some(line) = command.get("command_line").and_then(|v| v.as_str()) {
        return Some(line.to_owned());
    }
    let argv = command.get("argv")?.as_array()?;
    let parts: Option<Vec<&str>> = argv.iter().map(|a| a.as_str()).collect();
    parts.map(|p| p.join(" "))
}

/// `approval/scopes/list` — the Session's standing approval scopes.
///
/// Web caller: `src-web/apps/web/src/features/inspection/inspection-binding.ts`
/// (the inspection surface lists scopes beside the turn state; the client
/// sends `{session_id}`). Params/result are the octos-core types verbatim.
pub struct ApprovalScopesList;

impl Method for ApprovalScopesList {
    const NAME: &'static str = methods::APPROVAL_SCOPES_LIST;
    type Params = octos_core::ui_protocol::ApprovalScopesListParams;
    type Result = octos_core::ui_protocol::ApprovalScopesListResult;
}

/// `approval/decided` — the person (or a peer) decided a pending approval.
/// The parity matrix (`docs/parity/g-connection.csv`, row 7) says the lifecycle
/// notifications update the UI; the store marks the row decided so the sheet
/// stops showing it as pending. `auto_resolved` on the event separates a
/// manual decision from a policy one, so the store keeps that flag.
pub struct ApprovalDecidedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ApprovalDecidedHandler {
    const METHOD: &'static str = methods::APPROVAL_DECIDED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ApprovalDecided(decided) = notification {
            self.store.note_seen(Self::METHOD);
            self.store
                .domains
                .approval
                .settle(&decided.approval_id.0.to_string(), decided.auto_resolved);
        }
    }
}

/// `approval/cancelled` — the server cancelled a pending approval before any
/// client could respond (`reason` follows the open `approval_cancelled_reasons`
/// registry). The store drops the row from pending.
pub struct ApprovalCancelledHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ApprovalCancelledHandler {
    const METHOD: &'static str = methods::APPROVAL_CANCELLED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ApprovalCancelled(cancelled) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.approval.cancel(&cancelled.approval_id.0.to_string());
        }
    }
}

/// `approval/auto_resolved` — a policy auto-resolved a pending approval
/// (durable, replayed on reconnect). Treated as a decided row, marked
/// auto-resolved.
pub struct ApprovalAutoResolvedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ApprovalAutoResolvedHandler {
    const METHOD: &'static str = methods::APPROVAL_AUTO_RESOLVED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ApprovalAutoResolved(auto) = notification {
            self.store.note_seen(Self::METHOD);
            let id = auto.approval_id.0.to_string();
            self.store.domains.approval.settle(&id, true);
            // A6: the auto-resolve "toast" (parity row: "toast on
            // auto-resolve", 'Auto-approved' / 'Auto-denied') as a transcript
            // notice on the turn it unblocked. A policy decided without asking,
            // so no card ever showed: this line is the person's only record.
            // Deterministic id `approval:<id>`: a replayed (durable) event
            // updates the same row, never a second one.
            let approved = matches!(auto.decision, octos_core::ui_protocol::ApprovalDecision::Approve);
            let title = if approved { "Auto-approved" } else { "Auto-denied" };
            let body = format!("{} · matched the {} scope", auto.tool_name, auto.scope);
            self.store.domains.session.timeline.upsert_notice(
                &auto.session_id.0,
                Some(auto.turn_id.0.to_string()),
                &format!("approval:{id}"),
                format!("{title}: {body}"),
                serde_json::json!({
                    "title": title,
                    "message": body,
                    "kind": "approval_auto_resolved",
                    "approval_id": id,
                }),
            );
        }
    }
}

/// `user_question/respond` — the person's answer to a
/// `user_question/requested` event (UPCR-2026-023).
///
/// Web caller: `src-web/apps/web/src/features/session/session-interaction-ledger.ts`
/// (the answer is correlated by `question_id` on the question's own Session,
/// mirroring `approval/respond`). Params/result are the octos-core types.
pub struct UserQuestionRespond;

impl Method for UserQuestionRespond {
    const NAME: &'static str = methods::USER_QUESTION_RESPOND;
    type Params = octos_core::ui_protocol::UserQuestionRespondParams;
    type Result = octos_core::ui_protocol::UserQuestionRespondResult;
}

/// `approval/respond` — the person's decision on a pending approval.
///
/// The transport ALSO carries a typed `OutboundCommand::SendApprovalResponse`
/// for this method (`octos-app-transport/src/proto.rs:167`); this `Method`
/// makes it reachable through the client's ONE generic request path
/// (`Client::request`), which is how the web issues it
/// (`packages/client/src/client.ts`). Params/result are the octos-core types.
pub struct ApprovalRespond;

impl Method for ApprovalRespond {
    const NAME: &'static str = methods::APPROVAL_RESPOND;
    type Params = octos_core::ui_protocol::ApprovalRespondParams;
    type Result = octos_core::ui_protocol::ApprovalRespondResult;
}

/// `user_question/requested` — the server paused the turn to ask the person
/// (UPCR-2026-023, card #13 §3).
///
/// The web renders this as the question sheet and answers it with
/// `user_question/respond` (`src-web/apps/web/src/features/session/`,
/// `session-interaction-ledger.ts`). The store keeps the ONE outstanding
/// question so the sheet has something to render.
pub struct UserQuestionRequestedHandler {
    pub store: Arc<Store>,
}

impl NotificationHandler for UserQuestionRequestedHandler {
    const METHOD: &'static str = methods::USER_QUESTION_REQUESTED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::UserQuestionRequested(e) = notification {
            self.store.note_seen(Self::METHOD);
            self.store.domains.approval.set_question(PendingQuestion {
                question_id: e.question_id.0.to_string(),
                session_id: e.session_id.0.clone(),
                turn_id: e.turn_id.0.to_string(),
                title: e.title.clone(),
                body: e.body.clone(),
                questions: serde_json::to_value(&e.questions).unwrap_or(serde_json::Value::Null),
            });
        }
    }
}

/// Register this domain's notification handlers: the full approval lifecycle
/// the parity matrix names (`approval/requested` → `decided`/`cancelled`/
/// `auto_resolved`).
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(ApprovalRequestedHandler { store: store.clone() });
    reg.register(ApprovalDecidedHandler { store: store.clone() });
    reg.register(ApprovalCancelledHandler { store: store.clone() });
    reg.register(ApprovalAutoResolvedHandler { store: store.clone() });
    // Card #13 §3: the turn-pausing question (UPCR-2026-023).
    reg.register(UserQuestionRequestedHandler { store });
}
