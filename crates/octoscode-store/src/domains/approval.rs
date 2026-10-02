//! `approval` state: the pending approval the person must decide, and the
//! Session's standing approval scopes from `approval/scopes/list`.
//!
//! ## A20 — the interaction ledger (parity row 250)
//!
//! Every pending interaction (an approval or a user question) is keyed by its
//! EXACT origin: the owning SessionKey the wire named (`session_id`, plus its
//! split-wire `topic` — `"<session>#<topic>"`), the turn, the request id and
//! the transport authority GENERATION it was observed under — the web's
//! `SessionInteractionLedger` (`features/session/session-interaction-ledger.ts:83`,
//! records keyed `[endpoint, sessionId] + kind + turnId + requestId`,
//! `:115-125`). The rules ported here:
//!
//! * a topicless Session is a scope, never a wildcard for its topic children
//!   (`scope.ts:7-25` [`matches_session_scope`]); a frame whose topic
//!   conflicts with its own key has no owner ([`owner_of`]);
//! * a Session shows and answers ONLY its own interactions ([`Approvals::showing`],
//!   [`Approvals::question_for`]) — another Session's wait never takes it over;
//! * a response goes to the record's own owner, and only while that owner is
//!   the Session this client drives, the connection is ready and the record's
//!   generation is current ([`Approvals::authorize`], the web's `resolve`
//!   preflight `:400-414` — else [`STALE_GENERATION`], and nothing is sent);
//! * a restore from `session/hydrate` replaces exactly that owner's records
//!   (`restoreFromHydrate`, `:137-172`) — never another Session's — and never
//!   drops or revives what a NEWER live event settled ([`Approvals::restore`]).
use std::collections::HashMap;
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
/// valid; a row with no detail (a proof seed) has NO recorded origin, so it is
/// never shown and never answered (A20).
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
    /// A20 — the split-wire `topic` the request named (normalized: trimmed,
    /// empty = none). With `session_id` it is the record's exact owner
    /// ([`ApprovalDetail::owner`]).
    pub topic: Option<String>,
    /// A20 — the transport authority generation the request was observed
    /// under ([`Approvals::generation`]); a response is sent only while it is
    /// current.
    pub generation: u64,
    /// A20 — observed while its Session was not the one shown (the web's
    /// `unread`, the Waiting badge, `:189-194`).
    pub unread: bool,
    /// A20 — the ledger's observation order (set by the store).
    pub seq: u64,
}

impl ApprovalDetail {
    /// The owning SessionKey (`session_id`, or `session_id#topic`).
    pub fn owner(&self) -> String {
        owner_key(&self.session_id, self.topic.as_deref())
    }
}

/// The web's `StaleInteractionGenerationError` message
/// (`session-interaction-ledger.ts:73-80`): the response is refused before any
/// RPC because the record no longer belongs to the Session this client drives
/// (another Session is selected, the socket was replaced, or the record was
/// superseded or settled).
pub const STALE_GENERATION: &str = "This interaction no longer belongs to the current Session generation.";

/// Which kind of interaction a ledger record is (`SessionInteractionKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Approval,
    Question,
}

/// A20 — what a response's record became while its RPC was in flight
/// ([`Approvals::response_target`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseTarget {
    /// Still the record the response was authorized for.
    Current,
    /// The server's own lifecycle frame settled this very request first.
    Settled,
    /// Re-armed, superseded, or its socket was replaced: not this response's.
    Replaced,
}

/// A20 — `normalizedTopic` (`scope.ts:54-58`): trim, and empty means none
/// (rc11's ledger topic rule).
pub fn normalized_topic(topic: Option<&str>) -> Option<&str> {
    topic.map(str::trim).filter(|t| !t.is_empty())
}

/// A20 — `matchesSessionScope(expected, received, topic)` (`scope.ts:7-25`):
/// whether a frame naming `received` (+ `topic`) belongs to the Session keyed
/// `expected`. A topicless Session is a scope, never a wildcard for its topic
/// children; a `<base>#<topic>` Session accepts its base plus that exact topic.
pub fn matches_session_scope(expected: &str, received: &str, topic: Option<&str>) -> bool {
    let separator = expected.find('#');
    let expected_topic = separator.and_then(|i| normalized_topic(Some(&expected[i + 1..])));
    let received_topic = normalized_topic(topic);
    if received == expected {
        return received_topic.is_none() || received_topic == expected_topic;
    }
    match separator {
        Some(i) => expected_topic.is_some() && received == &expected[..i] && received_topic == expected_topic,
        None => false,
    }
}

/// A20 — the owning SessionKey a frame names: `session_id` when it carries no
/// topic, `<session_id>#<topic>` for a split-wire topic, or `None` when the
/// frame's key already names ANOTHER topic (no Session can own it — the
/// `"#review#nested"` vs `"nested"` case of `scope.test.ts:24-26`).
pub fn owner_of(session_id: &str, topic: Option<&str>) -> Option<String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return None;
    }
    let owner = match (normalized_topic(topic), session_id.find('#')) {
        (None, _) => session_id.to_owned(),
        (Some(t), None) => format!("{session_id}#{t}"),
        (Some(_), Some(_)) => session_id.to_owned(),
    };
    matches_session_scope(&owner, session_id, topic).then_some(owner)
}

/// [`owner_of`] for an already-admitted record (never `None`: a record is only
/// stored with an owner).
fn owner_key(session_id: &str, topic: Option<&str>) -> String {
    owner_of(session_id, topic).unwrap_or_else(|| session_id.to_owned())
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
    details: HashMap<String, ApprovalDetail>,
    /// The last `approval/scopes/list` result.
    scopes: Vec<StoredScope>,
    /// Card #13 / A20: the outstanding `user_question/requested` records, ONE
    /// per owning Session (the server pauses that Session's turn on it; a
    /// newer one for the same Session supersedes the older,
    /// `session-interaction-ledger.ts:199-210`). Before A20 this was one
    /// global slot: a second Session's question overwrote the first's.
    questions: Vec<PendingQuestion>,
    /// A20 — the transport authority generation: bumped whenever the socket
    /// is replaced, so a record observed on a retired socket can no longer be
    /// answered until a canonical restore re-arms it.
    generation: u64,
    /// A20 — the ledger's observation counter ([`ApprovalDetail::seq`]).
    seq: u64,
    /// A20 — request ids a live event settled (decided, cancelled,
    /// auto-resolved, answered, or their turn ended), by the sequence it
    /// happened at: a snapshot never revives them ([`Approvals::restore`]).
    settled: HashMap<String, u64>,
}

impl Inner {
    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn mark_settled(&mut self, id: &str) {
        let seq = self.next_seq();
        self.settled.insert(id.to_owned(), seq);
    }
}

/// One outstanding `user_question/requested` (card #13 §3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingQuestion {
    pub question_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub title: String,
    pub body: String,
    /// The structured questions, kept as JSON (the store needs no octos-core dep).
    pub questions: serde_json::Value,
    /// A20 — the split-wire topic (see [`ApprovalDetail::topic`]).
    pub topic: Option<String>,
    /// A20 — the authority generation it was observed under.
    pub generation: u64,
    /// A20 — observed while its Session was not the one shown.
    pub unread: bool,
    /// A20 — the ledger's observation order (set by the store).
    pub seq: u64,
}

impl PendingQuestion {
    /// The owning SessionKey (`session_id`, or `session_id#topic`).
    pub fn owner(&self) -> String {
        owner_key(&self.session_id, self.topic.as_deref())
    }
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

    /// #P4f2 row 7: the preview id of the oldest ACTIONABLE pending approval
    /// of `session` — what `D` binds to (web: the card that is showing
    /// decides, and the keydown handler reads THAT card's preview id). A20:
    /// scoped to the Session (the old global FIFO read another Session's).
    pub fn preview_id_for(&self, session: &str) -> Option<String> {
        self.showing(session).and_then(|(p, _)| p.preview_id)
    }

    pub fn pending(&self) -> Vec<PendingApproval> {
        self.inner.lock().unwrap().pending.clone()
    }

    /// Mark one decided by id; returns whether it was found.
    pub fn decide(&self, id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let found = match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.decided = true;
                true
            }
            None => false,
        };
        if found {
            i.mark_settled(id);
        }
        found
    }

    /// Settle a row on a lifecycle notification (`approval/decided` or
    /// `approval/auto_resolved`). Creates nothing: a decision for an id we
    /// never saw is ignored (the durable event can arrive without the request
    /// on this connection). Returns whether a row was updated.
    pub fn settle(&self, id: &str, auto_resolved: bool) -> bool {
        let mut i = self.inner.lock().unwrap();
        let found = match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.decided = true;
                a.auto_resolved = auto_resolved;
                true
            }
            None => false,
        };
        if found {
            i.mark_settled(id);
        }
        found
    }

    /// Mark a row cancelled (`approval/cancelled`). Returns whether a row was
    /// updated; an unknown id is ignored.
    pub fn cancel(&self, id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let found = match i.pending.iter_mut().find(|a| a.id == id) {
            Some(a) => {
                a.cancelled = true;
                true
            }
            None => false,
        };
        if found {
            i.mark_settled(id);
        }
        found
    }

    /// A20 — whether a resolution frame (`approval/decided`, `/cancelled`,
    /// `/auto_resolved`) names approval `id`'s exact origin: its
    /// `session_id` + `topic` are in the record's scope and its `turn_id`
    /// (when present) is the record's (`session-interaction-ledger.ts:291-316`:
    /// a foreign or wrong-turn resolution is ignored). An approval with no
    /// recorded origin is never settled by a scoped frame.
    pub fn resolution_matches(&self, id: &str, session_id: &str, topic: Option<&str>, turn_id: Option<&str>) -> bool {
        let i = self.inner.lock().unwrap();
        i.details.get(id).is_some_and(|d| {
            matches_session_scope(&d.owner(), session_id, topic) && turn_id.map_or(true, |t| t == d.turn_id)
        })
    }

    /// Replace the standing scope list with a fresh `approval/scopes/list`.
    pub fn set_scopes(&self, scopes: Vec<StoredScope>) {
        self.inner.lock().unwrap().scopes = scopes;
    }

    /// The last scope list read (empty until one lands).
    pub fn scopes(&self) -> Vec<StoredScope> {
        self.inner.lock().unwrap().scopes.clone()
    }

    /// Record an outstanding `user_question/requested` (card #13 §3). A20:
    /// keyed by its owning Session — a newer question for the SAME Session
    /// supersedes the older one (`session-interaction-ledger.ts:199-210`),
    /// another Session's question is untouched. A replayed observation of the
    /// same request on the same generation changes nothing (`:197-198`); a
    /// settled one never comes back.
    pub fn set_question(&self, question: PendingQuestion) {
        let mut i = self.inner.lock().unwrap();
        if i.settled.contains_key(&question.question_id) {
            return;
        }
        let owner = question.owner();
        if i.questions.iter().any(|q| {
            q.owner() == owner && q.question_id == question.question_id && q.generation == question.generation
        }) {
            return;
        }
        let mut question = question;
        question.seq = i.next_seq();
        i.questions.retain(|q| q.owner() != owner);
        i.questions.push(question);
    }

    /// The outstanding question of `owner` (its exact SessionKey), if any —
    /// what that Session's takeover shows and answers.
    pub fn question_for(&self, owner: &str) -> Option<PendingQuestion> {
        self.inner.lock().unwrap().questions.iter().find(|q| q.owner() == owner).cloned()
    }

    /// The newest outstanding question of ANY Session (diagnostics and the
    /// recorded-replay tests). Never a takeover's source: a Session shows only
    /// [`Approvals::question_for`] its own key.
    pub fn question(&self) -> Option<PendingQuestion> {
        self.inner.lock().unwrap().questions.iter().max_by_key(|q| q.seq).cloned()
    }

    /// Every outstanding question, oldest first (the Waiting badges).
    pub fn questions(&self) -> Vec<PendingQuestion> {
        let mut v = self.inner.lock().unwrap().questions.clone();
        v.sort_by_key(|q| q.seq);
        v
    }

    /// Clear every outstanding question (a test seam; production clears by id).
    pub fn clear_question(&self) -> bool {
        let mut i = self.inner.lock().unwrap();
        let had = !i.questions.is_empty();
        let ids: Vec<String> = i.questions.drain(..).map(|q| q.question_id).collect();
        for id in ids {
            i.mark_settled(&id);
        }
        had
    }

    // ---- A6: the takeover cards ------------------------------------------

    /// Record the card payload of one approval (the `approval/requested`
    /// handler, beside [`Approvals::request_with_preview`]). A20: stamps the
    /// ledger's observation order.
    pub fn set_detail(&self, id: &str, detail: ApprovalDetail) {
        let mut i = self.inner.lock().unwrap();
        let mut detail = detail;
        detail.seq = i.next_seq();
        i.details.insert(id.to_owned(), detail);
    }

    /// The card payload of one approval, if it carried one.
    pub fn detail(&self, id: &str) -> Option<ApprovalDetail> {
        self.inner.lock().unwrap().details.get(id).cloned()
    }

    /// A20 — observe one `approval/requested` under its exact origin (the
    /// web's `#store`, `session-interaction-ledger.ts:181-213`): the row and
    /// its card payload together. A replayed observation of the same request
    /// on the same generation changes nothing; an earlier generation's record
    /// of the same request is re-armed under the new one; a request a live
    /// event already settled never comes back. Returns whether it was stored.
    pub fn observe_approval(&self, id: &str, target: Option<String>, preview_id: Option<String>, detail: ApprovalDetail) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.settled.contains_key(id) {
            return false;
        }
        if let Some(d) = i.details.get(id) {
            if d.owner() == detail.owner() && d.turn_id == detail.turn_id && d.generation == detail.generation {
                return false;
            }
        }
        let mut detail = detail;
        detail.seq = i.next_seq();
        i.details.insert(id.to_owned(), detail);
        match i.pending.iter_mut().find(|a| a.id == id) {
            Some(existing) => {
                if preview_id.is_some() {
                    existing.preview_id = preview_id;
                }
            }
            None => i.pending.push(PendingApproval {
                id: id.to_owned(),
                target,
                decided: false,
                auto_resolved: false,
                cancelled: false,
                preview_id,
            }),
        }
        true
    }

    /// The approval card `session` shows: the OLDEST actionable row (not
    /// decided, not cancelled) whose recorded origin IS `session` — the same
    /// FIFO order the keyboard answers (`keys::oldest_pending_id`), scoped to
    /// the session so another session's wait never takes this one over (the
    /// web's session-scoped takeover, `ApprovalPanel.tsx:61`). A20: the origin
    /// is the exact owning key (`session#topic` for a split-wire topic), so a
    /// topic child's request never shows on its topicless base.
    pub fn showing(&self, session: &str) -> Option<(PendingApproval, ApprovalDetail)> {
        let i = self.inner.lock().unwrap();
        i.pending
            .iter()
            .filter(|a| !a.decided && !a.cancelled)
            .find_map(|a| {
                i.details
                    .get(&a.id)
                    .filter(|d| d.owner() == session)
                    .map(|d| (a.clone(), d.clone()))
            })
    }

    /// Actionable approvals of `session` (the strip's "Waiting for your
    /// approval" and the sidebar's waiting state read this, not the raw list,
    /// which keeps settled rows). A20: only rows whose recorded origin is
    /// `session` — a row with no recorded origin counted for EVERY Session.
    pub fn actionable_count(&self, session: &str) -> usize {
        let i = self.inner.lock().unwrap();
        i.pending
            .iter()
            .filter(|a| !a.decided && !a.cancelled)
            .filter(|a| i.details.get(&a.id).map(|d| d.owner() == session).unwrap_or(false))
            .count()
    }

    /// A20 — whether `owner` waits on the person (an actionable approval or
    /// an outstanding question of its own) — the sidebar's Waiting state for
    /// ANY Session, selected or not (`background-session-status.ts:9`).
    pub fn waiting(&self, owner: &str) -> bool {
        self.actionable_count(owner) > 0 || self.question_for(owner).is_some()
    }

    /// A20 — `markRead(config)` (`:341-350`): the Session is on screen, so its
    /// interactions are no longer unread. Only that Session's.
    pub fn mark_read(&self, owner: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let mut changed = false;
        for d in i.details.values_mut().filter(|d| d.owner() == owner && d.unread) {
            d.unread = false;
            changed = true;
        }
        for q in i.questions.iter_mut().filter(|q| q.owner() == owner && q.unread) {
            q.unread = false;
            changed = true;
        }
        changed
    }

    /// A20 — whether `owner` holds an unread interaction (the Waiting badge).
    pub fn unread(&self, owner: &str) -> bool {
        let i = self.inner.lock().unwrap();
        let open: Vec<&String> = i.pending.iter().filter(|a| !a.decided && !a.cancelled).map(|a| &a.id).collect();
        i.details.iter().any(|(id, d)| d.owner() == owner && d.unread && open.contains(&id))
            || i.questions.iter().any(|q| q.owner() == owner && q.unread)
    }

    /// A turn terminated: every interaction it was waiting on is gone (the
    /// web's `SessionInteractionLedger.settleTurn`,
    /// `session-interaction-ledger.ts:319-333`). Pending approvals of the turn
    /// become non-actionable and its question clears. Returns how many
    /// records were settled. Unscoped (by turn id alone): see
    /// [`Approvals::settle_turn_in`] for the Session-scoped form the turn
    /// handlers use.
    pub fn settle_turn(&self, turn_id: &str) -> usize {
        self.settle_turn_where(turn_id, |_| true)
    }

    /// A20 — `settleTurn(config, turnId)`: only the records of the Session
    /// the terminal names (its `session_id` + `topic`, in scope).
    pub fn settle_turn_in(&self, session_id: &str, topic: Option<&str>, turn_id: &str) -> usize {
        self.settle_turn_where(turn_id, |owner| matches_session_scope(owner, session_id, topic))
    }

    fn settle_turn_where(&self, turn_id: &str, owned: impl Fn(&str) -> bool) -> usize {
        let mut i = self.inner.lock().unwrap();
        let ids: Vec<String> = i
            .details
            .iter()
            .filter(|(_, d)| d.turn_id == turn_id && owned(&d.owner()))
            .map(|(id, _)| id.clone())
            .collect();
        let mut n = 0;
        let mut settled = Vec::new();
        for a in i.pending.iter_mut() {
            if !a.decided && !a.cancelled && ids.contains(&a.id) {
                a.cancelled = true;
                settled.push(a.id.clone());
                n += 1;
            }
        }
        let gone: Vec<String> = i
            .questions
            .iter()
            .filter(|q| q.turn_id == turn_id && owned(&q.owner()))
            .map(|q| q.question_id.clone())
            .collect();
        i.questions.retain(|q| !gone.contains(&q.question_id));
        n += gone.len();
        for id in settled.into_iter().chain(gone) {
            i.mark_settled(&id);
        }
        n
    }

    /// Clear the question only when it is still THIS one — a late answer
    /// receipt for a superseded question must not clear its successor (the
    /// web's identity re-check, `session-interaction-ledger.ts:395-407`).
    pub fn clear_question_if(&self, question_id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        let before = i.questions.len();
        i.questions.retain(|q| q.question_id != question_id);
        let cleared = i.questions.len() != before;
        if cleared {
            i.mark_settled(question_id);
        }
        cleared
    }

    // ---- A20: generation, authority, restore ------------------------------

    /// The current transport authority generation.
    pub fn generation(&self) -> u64 {
        self.inner.lock().unwrap().generation
    }

    /// The socket was replaced (a re-dial): every record observed so far
    /// belongs to a retired generation until a canonical restore re-arms it
    /// (`session-interaction-ledger.ts:429-452`, "resolve fails closed when the
    /// Session reconnected since observation"). Returns the new generation.
    pub fn advance_generation(&self) -> u64 {
        let mut i = self.inner.lock().unwrap();
        i.generation += 1;
        i.generation
    }

    /// The current observation sequence: a restore requested NOW keeps every
    /// record observed after it ([`Approvals::restore`]).
    pub fn observation_mark(&self) -> u64 {
        self.inner.lock().unwrap().seq
    }

    /// A20 — the `resolve` preflight (`session-interaction-ledger.ts:400-414`)
    /// for the interaction `request_id` of `kind`: it must still be pending
    /// and recorded for `owner`; `owner` must be the Session this client
    /// drives (`active`) over a `ready` connection; and the record's
    /// generation must be the current one. Returns that generation (the
    /// response's identity), or [`STALE_GENERATION`] — and then nothing may
    /// be sent.
    pub fn authorize(
        &self,
        kind: InteractionKind,
        owner: &str,
        request_id: &str,
        active: Option<&str>,
        ready: bool,
    ) -> Result<u64, &'static str> {
        let i = self.inner.lock().unwrap();
        let generation = match kind {
            InteractionKind::Approval => {
                let open = i.pending.iter().any(|a| a.id == request_id && !a.decided && !a.cancelled);
                match i.details.get(request_id).filter(|d| open && d.owner() == owner) {
                    Some(d) => d.generation,
                    None => return Err(STALE_GENERATION),
                }
            }
            InteractionKind::Question => {
                match i.questions.iter().find(|q| q.question_id == request_id && q.owner() == owner) {
                    Some(q) => q.generation,
                    None => return Err(STALE_GENERATION),
                }
            }
        };
        if !ready || active != Some(owner) || generation != i.generation {
            return Err(STALE_GENERATION);
        }
        Ok(generation)
    }

    /// A20 — whether the record `request_id` of `owner` is STILL the one a
    /// response was authorized for (same owner, same generation, still
    /// pending): the web's `isCurrent()` after the RPC (`:417-427`), so a
    /// late reply never settles a record a restore re-armed meanwhile
    /// ("does not clear a rehydrated same-ID request when the retired
    /// generation's response resolves", `.test.ts:266-286`).
    pub fn is_current(&self, kind: InteractionKind, owner: &str, request_id: &str, generation: u64) -> bool {
        let i = self.inner.lock().unwrap();
        if generation != i.generation {
            return false;
        }
        match kind {
            InteractionKind::Approval => {
                i.pending.iter().any(|a| a.id == request_id && !a.decided && !a.cancelled)
                    && i.details.get(request_id).is_some_and(|d| d.owner() == owner && d.generation == generation)
            }
            InteractionKind::Question => i
                .questions
                .iter()
                .any(|q| q.question_id == request_id && q.owner() == owner && q.generation == generation),
        }
    }

    /// A20 — after a response's RPC: whether its record is still the one it
    /// was authorized for ([`ResponseTarget::Current`]), was already settled
    /// by the server's own lifecycle frame for THIS request (the durable
    /// `approval/decided` can beat the RPC's reply — [`ResponseTarget::Settled`],
    /// nothing left to do), or was replaced: re-armed under another
    /// generation, superseded, or the socket changed
    /// ([`ResponseTarget::Replaced`] — the web's `!isCurrent()` return,
    /// `:463`, `:472`: no settle, no error).
    pub fn response_target(&self, kind: InteractionKind, owner: &str, request_id: &str, generation: u64) -> ResponseTarget {
        if self.is_current(kind, owner, request_id, generation) {
            return ResponseTarget::Current;
        }
        let i = self.inner.lock().unwrap();
        if generation != i.generation {
            return ResponseTarget::Replaced;
        }
        let settled = match kind {
            InteractionKind::Approval => {
                i.details.get(request_id).is_some_and(|d| d.owner() == owner && d.generation == generation)
                    && i.pending.iter().any(|a| a.id == request_id && (a.decided || a.cancelled))
            }
            InteractionKind::Question => {
                i.settled.contains_key(request_id) && !i.questions.iter().any(|q| q.question_id == request_id)
            }
        };
        if settled {
            ResponseTarget::Settled
        } else {
            ResponseTarget::Replaced
        }
    }

    /// A20 — `restoreFromHydrate(config, generation, hydrated)`
    /// (`session-interaction-ledger.ts:137-172`): the canonical snapshot of
    /// `owner`'s parked interactions REPLACES that owner's records — another
    /// Session's are never touched. The caller has already admitted each
    /// entry (parsed, in `owner`'s scope, negotiated). Two native guards, the
    /// LESSONS reconcile rule (the snapshot and live events are applied on
    /// different tasks here, in wire order on the web): a record observed
    /// AFTER the restore was requested (`since`, an [`Approvals::observation_mark`])
    /// is newer than the snapshot and stays; a request a live event settled
    /// is never revived. Returns how many interactions the snapshot armed.
    pub fn restore(
        &self,
        owner: &str,
        since: u64,
        approvals: Vec<(PendingApproval, ApprovalDetail)>,
        questions: Vec<PendingQuestion>,
    ) -> usize {
        let mut i = self.inner.lock().unwrap();
        // Drop the owner's records the snapshot supersedes.
        let stale: Vec<String> = i
            .details
            .iter()
            .filter(|(_, d)| d.owner() == owner && d.seq <= since)
            .map(|(id, _)| id.clone())
            .collect();
        i.pending.retain(|a| !stale.contains(&a.id));
        for id in &stale {
            i.details.remove(id);
        }
        i.questions.retain(|q| !(q.owner() == owner && q.seq <= since));
        let mut armed = 0;
        for (row, detail) in approvals {
            if detail.owner() != owner || i.settled.contains_key(&row.id) {
                continue;
            }
            if i.details.get(&row.id).is_some_and(|d| d.seq > since) {
                continue; // a newer live observation of the same request
            }
            let mut detail = detail;
            detail.seq = i.next_seq();
            i.details.insert(row.id.clone(), detail);
            if !i.pending.iter().any(|a| a.id == row.id) {
                i.pending.push(row);
            }
            armed += 1;
        }
        for q in questions {
            if q.owner() != owner || i.settled.contains_key(&q.question_id) {
                continue;
            }
            if i.questions.iter().any(|x| x.owner() == owner && x.seq > since) {
                continue; // a newer live question of this Session
            }
            let mut q = q;
            q.seq = i.next_seq();
            i.questions.retain(|x| x.owner() != owner);
            i.questions.push(q);
            armed += 1;
        }
        armed
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
        // A detail-less proof row has no recorded origin: never a card, never answered (A20).
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
            ..Default::default()
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
            ..Default::default()
        });
        assert!(!a.clear_question_if("q1"));
        assert!(a.clear_question_if("q2"));
    }
}

/// A20 — the web ledger's own cases (`session-interaction-ledger.test.ts`,
/// `scope.test.ts`) over the native store.
#[cfg(test)]
mod a20_ledger_tests {
    use super::*;

    fn approval(session: &str, topic: Option<&str>, turn: &str, generation: u64) -> ApprovalDetail {
        ApprovalDetail {
            session_id: session.into(),
            topic: topic.map(str::to_owned),
            turn_id: turn.into(),
            title: "Run?".into(),
            body: "pnpm test".into(),
            generation,
            ..Default::default()
        }
    }

    fn question(id: &str, session: &str, turn: &str, generation: u64) -> PendingQuestion {
        PendingQuestion {
            question_id: id.into(),
            session_id: session.into(),
            turn_id: turn.into(),
            title: "Choose".into(),
            generation,
            ..Default::default()
        }
    }

    fn row(id: &str) -> PendingApproval {
        PendingApproval { id: id.into(), target: None, decided: false, auto_resolved: false, cancelled: false, preview_id: None }
    }

    // scope.test.ts:10-27 + :101-130
    #[test]
    fn a_topicless_session_is_never_a_wildcard_for_its_topic_children() {
        let base = "coding:local:550e8400-e29b-41d4-a716-446655440000";
        assert!(!matches_session_scope(base, base, Some("foreign")));
        assert!(matches_session_scope(base, base, None));
        assert!(matches_session_scope(base, base, Some(" \t ")));
        assert!(matches_session_scope(&format!("{base}#review"), base, Some(" review ")));
        assert!(matches_session_scope(&format!("{base}#review"), &format!("{base}#review"), None));
        assert!(!matches_session_scope(&format!("{base}#review"), base, None));
        assert!(matches_session_scope(
            &format!("{base}#review#nested"),
            &format!("{base}#review#nested"),
            Some("review#nested")
        ));
        assert!(!matches_session_scope(&format!("{base}#review#nested"), &format!("{base}#review"), Some("nested")));
        assert!(matches_session_scope("profile:local:tui#coding", "profile:local:tui", Some("coding")));
        assert!(!matches_session_scope("profile:local:tui#coding", "profile:local:tui#coding", Some("review")));
        // The owner a frame names.
        assert_eq!(owner_of(base, None).as_deref(), Some(base));
        assert_eq!(owner_of(base, Some("peer-review")), Some(format!("{base}#peer-review")));
        assert_eq!(owner_of(&format!("{base}#review"), Some("review")), Some(format!("{base}#review")));
        assert_eq!(owner_of(&format!("{base}#review"), Some("nested")), None, "a conflicting topic has no owner");
        assert_eq!(owner_of("  ", None), None);
    }

    // "responds to split-wire approval with the confirmed full topic owner,
    // rejecting foreign and stale requests" (.test.ts:102-173)
    #[test]
    fn a_split_wire_request_belongs_to_its_full_topic_owner_only() {
        let a = Approvals::default();
        assert!(a.observe_approval("ap1", None, None, approval("s1", Some("peer-review"), "t1", 0)));
        assert!(a.showing("s1").is_none(), "the base Session never shows its topic child's request");
        assert!(a.showing("s1#peer-review").is_some());
        assert_eq!(
            a.authorize(InteractionKind::Approval, "s1", "ap1", Some("s1"), true),
            Err(STALE_GENERATION),
            "the base can never answer it"
        );
        assert_eq!(a.authorize(InteractionKind::Approval, "s1#peer-review", "ap1", Some("s1#peer-review"), true), Ok(0));
    }

    // "refuses a response after the session switched away and back, then
    // dispatches the re-armed record once" (.test.ts:557-571) +
    // "resolve fails closed when the Session reconnected since observation"
    #[test]
    fn a_response_needs_the_owner_selected_ready_and_current() {
        let a = Approvals::default();
        a.observe_approval("ap1", None, None, approval("s1", None, "t1", 0));
        // Another Session is selected: s1's request cannot be answered there.
        assert_eq!(a.authorize(InteractionKind::Approval, "s1", "ap1", Some("s2"), true), Err(STALE_GENERATION));
        // Not ready (recovering): refused.
        assert_eq!(a.authorize(InteractionKind::Approval, "s1", "ap1", Some("s1"), false), Err(STALE_GENERATION));
        assert_eq!(a.authorize(InteractionKind::Approval, "s1", "ap1", Some("s1"), true), Ok(0));
        // The socket was replaced: the old observation is stale…
        assert_eq!(a.advance_generation(), 1);
        assert_eq!(a.authorize(InteractionKind::Approval, "s1", "ap1", Some("s1"), true), Err(STALE_GENERATION));
        // …until the canonical restore re-arms it under the new generation.
        let mark = a.observation_mark();
        assert_eq!(a.restore("s1", mark, vec![(row("ap1"), approval("s1", None, "t1", 1))], vec![]), 1);
        assert_eq!(a.authorize(InteractionKind::Approval, "s1", "ap1", Some("s1"), true), Ok(1));
        // A late reply of the retired generation never settles the re-armed record.
        assert!(!a.is_current(InteractionKind::Approval, "s1", "ap1", 0));
        assert!(a.is_current(InteractionKind::Approval, "s1", "ap1", 1));
    }

    // "keeps a non-selected session's pending interaction visible and unread"
    // (.test.ts:584-612) + the question overwrite the single slot allowed.
    #[test]
    fn each_session_keeps_its_own_question_and_unread_state() {
        let a = Approvals::default();
        let mut qx = question("qx", "sx", "tx", 0);
        qx.unread = true;
        a.set_question(qx);
        a.set_question(question("qy", "sy", "ty", 0));
        assert_eq!(a.question_for("sx").map(|q| q.question_id).as_deref(), Some("qx"), "Y's question never overwrites X's");
        assert_eq!(a.question_for("sy").map(|q| q.question_id).as_deref(), Some("qy"));
        assert!(a.waiting("sx") && a.waiting("sy") && !a.waiting("sz"));
        assert!(a.unread("sx"));
        assert!(!a.mark_read("sy"), "selecting Y never reads X");
        assert!(a.unread("sx"));
        assert!(a.mark_read("sx"));
        assert!(!a.unread("sx"));
        // A newer question of the SAME Session supersedes its older one.
        a.set_question(question("qx2", "sx", "tx", 0));
        assert_eq!(a.question_for("sx").map(|q| q.question_id).as_deref(), Some("qx2"));
        assert_eq!(a.authorize(InteractionKind::Question, "sx", "qx", Some("sx"), true), Err(STALE_GENERATION));
    }

    // "settleTurn drops the record for that turn only" (.test.ts:472-502)
    #[test]
    fn a_turn_terminal_settles_only_its_own_sessions_records() {
        let a = Approvals::default();
        a.observe_approval("ap-1", None, None, approval("s1", None, "t1", 0));
        a.observe_approval("ap-2", None, None, approval("s2", None, "t1", 0));
        a.set_question(question("q-2", "s2", "t1", 0));
        assert_eq!(a.settle_turn_in("s1", None, "t1"), 1);
        assert!(a.showing("s1").is_none());
        assert!(a.showing("s2").is_some(), "the same turn id in another Session is untouched");
        assert!(a.question_for("s2").is_some());
    }

    // "rejects resolution … and ignores foreign or wrong-turn resolution
    // notifications" (.test.ts:288-326)
    #[test]
    fn a_resolution_frame_must_name_the_exact_origin_and_turn() {
        let a = Approvals::default();
        a.observe_approval("ap1", None, None, approval("s1", None, "t1", 0));
        assert!(!a.resolution_matches("ap1", "other", None, Some("t1")));
        assert!(!a.resolution_matches("ap1", "s1", None, Some("other")));
        assert!(!a.resolution_matches("ap1", "s1", Some("peer"), Some("t1")));
        assert!(a.resolution_matches("ap1", "s1", None, Some("t1")));
        assert!(a.resolution_matches("ap1", "s1", None, None));
        assert!(!a.resolution_matches("unknown", "s1", None, None));
    }

    // "restores full parsed approval and question payloads … exact Waiting
    // records" (.test.ts:174-196): a restore replaces only its owner's records.
    #[test]
    fn a_restore_replaces_only_its_owners_records_and_never_revives_a_settled_one() {
        let a = Approvals::default();
        a.observe_approval("ap-y", None, None, approval("sy", None, "ty", 0));
        a.observe_approval("ap-x-old", None, None, approval("sx", None, "tx0", 0));
        let mark = a.observation_mark();
        // A live request of X observed AFTER the snapshot was requested stays.
        a.observe_approval("ap-x-live", None, None, approval("sx", None, "tx2", 0));
        // A live decision after the request: the snapshot cannot revive it.
        a.observe_approval("ap-x-decided", None, None, approval("sx", None, "tx3", 0));
        a.settle("ap-x-decided", false);
        let armed = a.restore(
            "sx",
            mark,
            vec![
                (row("ap-x"), approval("sx", None, "tx", 0)),
                (row("ap-x-decided"), approval("sx", None, "tx3", 0)),
                (row("ap-foreign"), approval("sy", None, "ty", 0)),
            ],
            vec![question("q-x", "sx", "tx", 0)],
        );
        assert_eq!(armed, 2, "X's parked approval and question");
        assert!(a.detail("ap-x-old").is_none(), "the snapshot superseded X's older record");
        assert!(a.showing("sy").is_some(), "Y's record is untouched");
        assert_eq!(a.actionable_count("sx"), 2, "the parked one and the newer live one");
        assert!(a.pending().iter().any(|p| p.id == "ap-x-decided" && p.decided), "never revived");
        assert!(a.detail("ap-foreign").is_none(), "a foreign entry is never armed");
        assert_eq!(a.question_for("sx").map(|q| q.question_id).as_deref(), Some("q-x"));
    }

    // "ignores duplicate replay … while its exact response remains in flight"
    // (.test.ts:328-349): a replayed observation never re-stamps the record.
    #[test]
    fn a_replayed_observation_of_the_same_request_changes_nothing() {
        let a = Approvals::default();
        assert!(a.observe_approval("ap1", None, None, approval("s1", None, "t1", 0)));
        let seq = a.detail("ap1").unwrap().seq;
        assert!(!a.observe_approval("ap1", None, None, approval("s1", None, "t1", 0)));
        assert_eq!(a.detail("ap1").unwrap().seq, seq);
        // A decided request never comes back from a late replay.
        a.decide("ap1");
        assert!(!a.observe_approval("ap1", None, None, approval("s1", None, "t1", 1)));
        assert!(a.showing("s1").is_none());
    }

    // The durable `approval/decided` of THIS request can land before the
    // response's reply: settled, not replaced; a re-armed record or a new
    // socket is replaced (the web's `!isCurrent()` return).
    #[test]
    fn a_response_target_tells_settled_from_replaced() {
        let a = Approvals::default();
        a.observe_approval("ap1", None, None, approval("s1", None, "t1", 0));
        assert_eq!(a.response_target(InteractionKind::Approval, "s1", "ap1", 0), ResponseTarget::Current);
        a.settle("ap1", false);
        assert_eq!(a.response_target(InteractionKind::Approval, "s1", "ap1", 0), ResponseTarget::Settled);
        assert_eq!(a.response_target(InteractionKind::Approval, "s2", "ap1", 0), ResponseTarget::Replaced);
        a.set_question(question("q1", "s1", "t1", 0));
        assert!(a.clear_question_if("q1"));
        assert_eq!(a.response_target(InteractionKind::Question, "s1", "q1", 0), ResponseTarget::Settled);
        a.observe_approval("ap2", None, None, approval("s1", None, "t2", 0));
        a.advance_generation();
        assert_eq!(a.response_target(InteractionKind::Approval, "s1", "ap2", 0), ResponseTarget::Replaced);
    }

    // A row with no recorded origin (a proof seed) counts for no Session.
    #[test]
    fn an_unattributed_row_waits_for_no_session() {
        let a = Approvals::default();
        a.request("seed", None);
        assert_eq!(a.actionable_count("s1"), 0);
        assert!(!a.waiting("s1"));
        assert!(a.showing("s1").is_none());
    }
}
