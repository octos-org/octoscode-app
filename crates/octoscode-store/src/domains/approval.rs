//! `approval` state: the pending approval the person must decide, and the
//! Session's standing approval scopes from `approval/scopes/list`.
use std::sync::Mutex;

/// One pending approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub id: String,
    pub target: Option<String>,
    pub decided: bool,
    /// Set when the decision came from a policy (`approval/auto_resolved`)
    /// rather than a person. `false` until decided.
    pub auto_resolved: bool,
    /// Set when the server cancelled the approval before any client could
    /// respond (`approval/cancelled`). A cancelled row is no longer actionable.
    pub cancelled: bool,
    /// #P4f2 row 7: the diff preview id the approval PAYLOAD carries
    /// (`typedDetails.diff.preview_id`, web
    /// `approvalDiffPreviewId` — `packages/client/src/interaction.ts:94-102`).
    /// `D` is bound only when this is present, exactly like the web's
    /// `if (key === "d" && previewId && onReviewDiff)`
    /// (`ApprovalPanel.tsx:45`). `None` = a non-diff approval, and D stays
    /// inert.
    pub preview_id: Option<String>,
}

/// One standing approval scope, flattened from `ApprovalScopeEntry`
/// (`approval/scopes/list`): which scope, how it matches, the decision, and
/// the bound turn for a `turn`-scoped entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredScope {
    pub scope: String,
    pub scope_match: String,
    pub decision: String,
    pub turn_id: Option<String>,
}

/// A6 — what the approval takeover card renders for one pending approval:
/// the web's parsed `ApprovalRequested` (`packages/client/src/interaction.ts:
/// 41-79`) reduced to the fields `ApprovalPanel.tsx:58-87` draws, plus the
/// owning session/turn the takeover is scoped to. Kept beside (not inside)
/// [`PendingApproval`] so the many existing constructions of that row stay
/// valid; a row with no detail (a proof seed) is still keyboard-answerable
/// but never draws a card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalDetail {
    pub session_id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub title: String,
    pub body: String,
    /// `approval_kind` (`command`, `diff`, …).
    pub kind: Option<String>,
    /// `risk` (`low`, `medium`, `high`, `unspecified`, …) — shown verbatim.
    pub risk: Option<String>,
    /// `typed_details.command`: `command_line`, else the `argv` joined by
    /// spaces (`ApprovalPanel.tsx:131-139`); `None` when neither is a string
    /// (list).
    pub command: Option<String>,
}

/// The approval domain.
#[derive(Debug, Default)]
pub struct Approvals {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    pending: Vec<PendingApproval>,
    /// A6: the takeover card's payload per approval id.
    details: std::collections::HashMap<String, ApprovalDetail>,
    /// The last `approval/scopes/list` result.
    scopes: Vec<StoredScope>,
    /// Card #13: the outstanding `user_question/requested`, if any
    /// (UPCR-2026-023). One at a time — the server pauses the turn on it.
    question: Option<PendingQuestion>,
}

/// One outstanding `user_question/requested` (card #13 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingQuestion {
    pub question_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub title: String,
    pub body: String,
    /// The structured questions, kept as JSON (the store needs no octos-core dep).
    pub questions: serde_json::Value,
}

impl Approvals {
    pub fn push(&self, approval: PendingApproval) {
        self.inner.lock().unwrap().pending.push(approval);
    }

    /// Create a pending row from the wire ids alone (the common case: a fresh
    /// `approval/requested`). Keeps existing rows for the same id.
    pub fn request(&self, id: &str, target: Option<String>) {
        self.request_with_preview(id, target, None)
    }

    /// #P4f2 row 7: as [`Approvals::request`], plus the diff preview id the
    /// approval payload carried. `None` = a non-diff approval.
    pub fn request_with_preview(
        &self,
        id: &str,
        target: Option<String>,
        preview_id: Option<String>,
    ) {
        let mut i = self.inner.lock().unwrap();
        if let Some(existing) = i.pending.iter_mut().find(|a| a.id == id) {
            // An update to the same id may still carry the preview id, but must
            // never clear a decision already recorded.
            if preview_id.is_some() {
                existing.preview_id = preview_id;
            }
            return;
        }
        i.pending.push(PendingApproval {
            id: id.to_owned(),
            target,
            decided: false,
            auto_resolved: false,
            cancelled: false,
            preview_id,
        });
    }

    /// #P4f2 row 7: the preview id of the oldest ACTIONABLE pending approval —
    /// what `D` binds to (web: the card that is showing decides, and the
    /// keydown handler reads THAT card's preview id).
    pub fn oldest_preview_id(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .pending
            .iter()
            .find(|a| !a.decided && !a.cancelled)
            .and_then(|a| a.preview_id.clone())
    }

    pub fn pending(&self) -> Vec<PendingApproval> {
        self.inner.lock().unwrap().pending.clone()
    }

    /// Mark one decided by id; returns whether it was found.
    pub fn decide(&self, id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.decided = true;
                true
            }
            None => false,
        }
    }

    /// Settle a row on a lifecycle notification (`approval/decided` or
    /// `approval/auto_resolved`). Creates nothing: a decision for an id we
    /// never saw is ignored (the durable event can arrive without the request
    /// on this connection). Returns whether a row was updated.
    pub fn settle(&self, id: &str, auto_resolved: bool) -> bool {
        let mut i = self.inner.lock().unwrap();
        match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.decided = true;
                a.auto_resolved = auto_resolved;
                true
            }
            None => false,
        }
    }

    /// Mark a row cancelled (`approval/cancelled`). Returns whether a row was
    /// updated; an unknown id is ignored.
    pub fn cancel(&self, id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.cancelled = true;
                true
            }
            None => false,
        }
    }

    /// Replace the standing scope list with a fresh `approval/scopes/list`.
    pub fn set_scopes(&self, scopes: Vec<StoredScope>) {
        self.inner.lock().unwrap().scopes = scopes;
    }

    /// The last scope list read (empty until one lands).
    pub fn scopes(&self) -> Vec<StoredScope> {
        self.inner.lock().unwrap().scopes.clone()
    }

    /// Record the outstanding `user_question/requested` (card #13 §3).
    pub fn set_question(&self, question: PendingQuestion) {
        self.inner.lock().unwrap().question = Some(question);
    }

    /// The outstanding question, if any.
    pub fn question(&self) -> Option<PendingQuestion> {
        self.inner.lock().unwrap().question.clone()
    }

    /// Clear the outstanding question (a `user_question/respond` was sent, or
    /// the turn ended).
    pub fn clear_question(&self) -> bool {
        self.inner.lock().unwrap().question.take().is_some()
    }

    // ---- A6: the takeover cards ------------------------------------------

    /// Record the card payload of one approval (the `approval/requested`
    /// handler, beside [`Approvals::request_with_preview`]).
    pub fn set_detail(&self, id: &str, detail: ApprovalDetail) {
        self.inner.lock().unwrap().details.insert(id.to_owned(), detail);
    }

    /// The card payload of one approval, if it carried one.
    pub fn detail(&self, id: &str) -> Option<ApprovalDetail> {
        self.inner.lock().unwrap().details.get(id).cloned()
    }

    /// The approval card `session` shows: the OLDEST actionable row (not
    /// decided, not cancelled) whose payload belongs to `session` — the same
    /// FIFO order the keyboard answers (`keys::oldest_pending_id`), scoped to
    /// the session so another session's wait never takes this one over (the
    /// web's session-scoped takeover, `ApprovalPanel.tsx:61`).
    pub fn showing(&self, session: &str) -> Option<(PendingApproval, ApprovalDetail)> {
        let i = self.inner.lock().unwrap();
        i.pending
            .iter()
            .filter(|a| !a.decided && !a.cancelled)
            .find_map(|a| {
                i.details
                    .get(&a.id)
                    .filter(|d| d.session_id == session)
                    .map(|d| (a.clone(), d.clone()))
            })
    }

    /// Actionable approvals of `session` (the strip's "Waiting for your
    /// approval" and the sidebar's waiting state read this, not the raw list,
    /// which keeps settled rows).
    pub fn actionable_count(&self, session: &str) -> usize {
        let i = self.inner.lock().unwrap();
        i.pending
            .iter()
            .filter(|a| !a.decided && !a.cancelled)
            .filter(|a| i.details.get(&a.id).map(|d| d.session_id == session).unwrap_or(true))
            .count()
    }

    /// A turn terminated: every interaction it was waiting on is gone (the
    /// web's `SessionInteractionLedger.settleTurn`,
    /// `session-interaction-ledger.ts:307-321`). Pending approvals of the turn
    /// become non-actionable and its question clears. Returns how many
    /// records were settled.
    pub fn settle_turn(&self, turn_id: &str) -> usize {
        let mut i = self.inner.lock().unwrap();
        let ids: Vec<String> = i
            .details
            .iter()
            .filter(|(_, d)| d.turn_id == turn_id)
            .map(|(id, _)| id.clone())
            .collect();
        let mut n = 0;
        for a in i.pending.iter_mut() {
            if !a.decided && !a.cancelled && ids.contains(&a.id) {
                a.cancelled = true;
                n += 1;
            }
        }
        if i.question.as_ref().map(|q| q.turn_id == turn_id).unwrap_or(false) {
            i.question = None;
            n += 1;
        }
        n
    }

    /// Clear the question only when it is still THIS one — a late answer
    /// receipt for a superseded question must not clear its successor (the
    /// web's identity re-check, `session-interaction-ledger.ts:395-407`).
    pub fn clear_question_if(&self, question_id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.question.as_ref().map(|q| q.question_id == question_id).unwrap_or(false) {
            i.question = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod a6_tests {
    use super::*;

    fn detail(session: &str, turn: &str) -> ApprovalDetail {
        ApprovalDetail {
            session_id: session.into(),
            turn_id: turn.into(),
            tool_name: "shell".into(),
            title: "Approve command".into(),
            body: "printf ok".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_showing_card_is_the_sessions_oldest_actionable_row() {
        let a = Approvals::default();
        a.request("a1", None);
        a.set_detail("a1", detail("s2", "t1"));
        a.request("a2", None);
        a.set_detail("a2", detail("s1", "t2"));
        a.request("a3", None);
        a.set_detail("a3", detail("s1", "t3"));
        // Another session's wait never takes over s1.
        assert_eq!(a.showing("s1").map(|(p, _)| p.id).as_deref(), Some("a2"));
        assert_eq!(a.showing("s2").map(|(p, _)| p.id).as_deref(), Some("a1"));
        a.settle("a2", false);
        assert_eq!(a.showing("s1").map(|(p, _)| p.id).as_deref(), Some("a3"));
        assert_eq!(a.actionable_count("s1"), 1);
        // A detail-less proof row is answerable but never a card.
        a.request("seed", None);
        assert!(a.showing("nowhere").is_none());
    }

    #[test]
    fn a_turn_terminal_settles_its_approvals_and_its_question() {
        let a = Approvals::default();
        a.request("a1", None);
        a.set_detail("a1", detail("s1", "t1"));
        a.request("a2", None);
        a.set_detail("a2", detail("s1", "t2"));
        a.set_question(PendingQuestion {
            question_id: "q1".into(),
            session_id: "s1".into(),
            turn_id: "t1".into(),
            title: "Pick".into(),
            body: String::new(),
            questions: serde_json::Value::Null,
        });
        assert_eq!(a.settle_turn("t1"), 2, "the approval and the question of t1");
        assert!(a.question().is_none());
        assert_eq!(a.showing("s1").map(|(p, _)| p.id).as_deref(), Some("a2"), "t2 still waits");
        // A receipt for another question never clears the current one.
        a.set_question(PendingQuestion {
            question_id: "q2".into(),
            session_id: "s1".into(),
            turn_id: "t2".into(),
            title: "Pick".into(),
            body: String::new(),
            questions: serde_json::Value::Null,
        });
        assert!(!a.clear_question_if("q1"));
        assert!(a.clear_question_if("q2"));
    }
}
