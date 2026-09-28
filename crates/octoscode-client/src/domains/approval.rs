//! `approval/*` — the approval sheet (request, decide, cancel).
//!
//! Requests implemented here: `approval/scopes/list`, `user_question/respond`.
//! `approval/respond` stays on the transport's typed `OutboundCommand` (it
//! carries a oneshot reply and is gated by the approval feature). Notifications
//! this file names: `approval/requested` (stored), and `approval/decided`,
//! `approval/cancelled`, `approval/auto_resolved` (deliberately left to the
//! registry's tolerated-unknown arm — see `register`).
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::approval::PendingApproval;
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
            self.store.domains.approval.push(PendingApproval {
                id: requested.approval_id.0.to_string(),
                target: Some(requested.tool_name.clone()),
                decided: false,
            });
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

/// Register this domain's notification handlers.
///
/// NOTE: `approval/decided`, `approval/cancelled` and `approval/auto_resolved`
/// are deliberately NOT registered: `crates/octoscode-client/tests/client_core.rs:89`
/// pins `!reg.handles(methods::APPROVAL_DECIDED)`, and that shared test is
/// outside this lane's owned files. Unclaimed notifications go to the
/// registry's tolerated-unknown arm (`registry.rs`, `debug!` by method name),
/// never silently dropped (RULES #6, 8.8 condition 7). If the lifecycle
/// notifications should be stored, the one-line change in `client_core.rs:89`
/// is needed; see the F5 report.
pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(ApprovalRequestedHandler { store });
}
