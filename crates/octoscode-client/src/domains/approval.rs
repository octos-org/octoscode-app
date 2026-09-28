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
            self.store
                .domains
                .approval
                .request(&requested.approval_id.0.to_string(), Some(requested.tool_name.clone()));
        }
    }
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
            self.store
                .domains
                .approval
                .settle(&auto.approval_id.0.to_string(), true);
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

/// Register this domain's notification handlers: the full approval lifecycle
/// the parity matrix names (`approval/requested` → `decided`/`cancelled`/
/// `auto_resolved`).
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(ApprovalRequestedHandler { store: store.clone() });
    reg.register(ApprovalDecidedHandler { store: store.clone() });
    reg.register(ApprovalCancelledHandler { store: store.clone() });
    reg.register(ApprovalAutoResolvedHandler { store });
}
