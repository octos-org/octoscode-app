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
///
/// A20 — observed under its EXACT origin (the web's `SessionInteractionLedger`
/// `#observeNotification`, `session-interaction-ledger.ts:229-263`): the
/// owning SessionKey is the frame's `session_id` plus its split-wire `topic`
/// ([`interaction_owner`]); a frame whose topic names a Session this client
/// does not hold (a foreign topic on an ordinary owner) or whose key conflicts
/// with its topic is rejected, logged by name, never stored; a server that
/// advertised its methods without `approval/respond` gets no card it could
/// never answer. The record carries the authority generation and whether its
/// Session was on screen (unread = the Waiting badge).
pub struct ApprovalRequestedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for ApprovalRequestedHandler {
    const METHOD: &'static str = methods::APPROVAL_REQUESTED;
    fn handle(&self, notification: &UiNotification) {
        if let UiNotification::ApprovalRequested(requested) = notification {
            self.store.note_seen(Self::METHOD);
            let Some(owner) = interaction_owner(&self.store, &requested.session_id.0, requested.topic.as_deref()) else {
                ::log::warn!(
                    "octoscode: approval/requested {} for a foreign session/topic ({} / {:?}) rejected",
                    requested.approval_id.0,
                    requested.session_id.0,
                    requested.topic
                );
                return;
            };
            if !negotiated(&self.store, "approval/respond", None) {
                ::log::warn!("octoscode: approval/requested without an advertised approval/respond rejected");
                return;
            }
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
            // A6: the takeover card's payload (`ApprovalPanel.tsx:58-87`:
            // title, body, risk, tool, kind, the typed command), scoped to
            // the session + turn the request names. A20: with its exact
            // origin, generation and unread state, as ONE ledger record.
            let mut detail = approval_detail(requested);
            detail.generation = self.store.domains.approval.generation();
            detail.unread = self.store.active_session().as_deref() != Some(owner.as_str());
            self.store
                .domains
                .approval
                .observe_approval(&id, Some(requested.tool_name.clone()), preview_id, detail);
        }
    }
}

/// A20 — the owning SessionKey of an interaction frame, or `None` when it
/// must be rejected: the frame's key conflicts with its topic, or it names a
/// TOPIC owner this client does not hold. A topicless Session is a scope,
/// never a wildcard for its topic children (`scope.ts:7-25`): a request for
/// `<session>` with topic `peer-review` belongs to `<session>#peer-review`
/// only, which must be the Session shown or one this client opened.
pub fn interaction_owner(store: &Store, session_id: &str, topic: Option<&str>) -> Option<String> {
    use octoscode_store::domains::approval::{normalized_topic, owner_of};
    let owner = owner_of(session_id, topic)?;
    if normalized_topic(topic).is_none() && !owner.contains('#') {
        return Some(owner);
    }
    let held = store.active_session().as_deref() == Some(owner.as_str()) || store.sessions().iter().any(|s| s.id == owner);
    held.then_some(owner)
}

/// A20 — `supportsMethod(capabilities, method)` (+ `supportsFeature` for a
/// question, `session-interaction-ledger.ts:248, 274-275`) against what the
/// server advertised. Before any advertisement is known (a replayed frame
/// with no open reply) nothing is refused here: the takeover keeps its own
/// gate, and an answer fails visibly.
pub fn negotiated(store: &Store, method: &str, feature: Option<&str>) -> bool {
    let methods = store.domains.config.supported_methods();
    if methods.is_empty() {
        return true;
    }
    methods.iter().any(|m| m == method) && feature.map_or(true, |f| feature_advertised(store, f))
}

/// A20 — `supportsFeature`: the negotiated capability set, or the open
/// reply's `supported_features`.
fn feature_advertised(store: &Store, feature: &str) -> bool {
    store.domains.config.has_capability(feature) || store.domains.config.supported_features().iter().any(|f| f == feature)
}

/// A20 — `restoreFromHydrate(config, generation, hydrated, {background})`
/// (`session-interaction-ledger.ts:137-172`): the parked interactions of the
/// canonical `session/hydrate` reply of `owner`, each admitted on its own the
/// way a live frame is — a MALFORMED entry (it does not parse as the
/// protocol's `ApprovalRequestedEvent` / `UserQuestionRequestedEvent`), a
/// FOREIGN one (its `session_id` + `topic` are outside `owner`'s scope,
/// `scope.ts:7-25`) and an UNNEGOTIATED one (the server did not advertise the
/// answer method, or `user_question.v1` for a question) are dropped, never the
/// whole reply ("rejects malformed, foreign-topic and unnegotiated hydrate
/// requests", `session-interaction-ledger.test.ts:198-217`). What remains
/// replaces exactly `owner`'s records ([`Approvals::restore`]), attributed to
/// that asker, under `generation`. `since` is the ledger mark taken when the
/// hydrate was requested. Returns (approvals, questions) armed.
///
/// [`Approvals::restore`]: octoscode_store::domains::approval::Approvals::restore
pub fn restore_from_hydrate(
    store: &Store,
    owner: &str,
    since: u64,
    generation: u64,
    hydrated: &serde_json::Value,
    background: bool,
) -> (usize, usize) {
    use octoscode_store::domains::approval::{matches_session_scope, PendingApproval};
    let methods = store.domains.config.supported_methods();
    let approve = methods.iter().any(|m| m == "approval/respond");
    let ask = methods.iter().any(|m| m == "user_question/respond") && feature_advertised(store, "user_question.v1");
    let entries = |key: &str| hydrated.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut approvals = Vec::new();
    for raw in entries("pending_approvals") {
        let Ok(e) = serde_json::from_value::<octos_core::ui_protocol::ApprovalRequestedEvent>(raw) else {
            ::log::warn!("octoscode: a malformed parked approval of {owner} dropped");
            continue;
        };
        if !matches_session_scope(owner, &e.session_id.0, e.topic.as_deref()) {
            ::log::warn!("octoscode: parked approval {} of a foreign session/topic dropped from {owner}", e.approval_id.0);
            continue;
        }
        if !approve {
            ::log::warn!("octoscode: parked approval {} dropped: approval/respond is not advertised", e.approval_id.0);
            continue;
        }
        let preview_id = e
            .typed_details
            .as_ref()
            .and_then(|d| d.diff.as_ref())
            .map(|d| crate::protocol_id::preview_id_string(&d.preview_id))
            .filter(|id| crate::protocol_id::is_protocol_uuid(&serde_json::json!(id)));
        let mut detail = approval_detail(&e);
        // The record is the CONFIG's — the asker's exact key — whatever split
        // form (`<base>` + topic, or `<base>#<topic>`) the entry used.
        detail.session_id = owner.to_owned();
        detail.topic = None;
        detail.generation = generation;
        detail.unread = background;
        let row = PendingApproval {
            id: e.approval_id.0.to_string(),
            target: Some(e.tool_name.clone()),
            decided: false,
            auto_resolved: false,
            cancelled: false,
            preview_id,
        };
        approvals.push((row, detail));
    }
    let mut questions = Vec::new();
    for raw in entries("pending_questions") {
        let Ok(q) = serde_json::from_value::<octos_core::ui_protocol::UserQuestionRequestedEvent>(raw) else {
            ::log::warn!("octoscode: a malformed parked question of {owner} dropped");
            continue;
        };
        if !matches_session_scope(owner, &q.session_id.0, q.topic.as_deref()) {
            ::log::warn!("octoscode: parked question {} of a foreign session/topic dropped from {owner}", q.question_id.0);
            continue;
        }
        if !ask {
            ::log::warn!("octoscode: parked question {} dropped: user_question/respond is not negotiated", q.question_id.0);
            continue;
        }
        questions.push(PendingQuestion {
            question_id: q.question_id.0.to_string(),
            session_id: owner.to_owned(),
            turn_id: q.turn_id.0.to_string(),
            title: q.title.clone(),
            body: q.body.clone(),
            questions: serde_json::to_value(&q.questions).unwrap_or(serde_json::Value::Null),
            topic: None,
            generation,
            unread: background,
            seq: 0,
        });
    }
    let (a, q) = (approvals.len(), questions.len());
    let armed = store.domains.approval.restore(owner, since, approvals, questions);
    if armed < a + q {
        ::log::info!("octoscode: restore of {owner}: {armed} of {} admitted interactions armed (settled or newer kept)", a + q);
    }
    (a, q)
}

/// A20 — whether a resolution frame may settle approval `id`: a record with
/// a recorded origin settles only from a frame in its scope and of its turn
/// (`session-interaction-ledger.ts:291-316`: a foreign or wrong-turn
/// resolution is ignored); a row with none (a proof seed) settles by its id.
fn resolves(store: &Store, id: &str, session_id: &str, topic: Option<&str>, turn_id: &str) -> bool {
    match store.domains.approval.detail(id) {
        Some(_) => store.domains.approval.resolution_matches(id, session_id, topic, Some(turn_id)),
        None => true,
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
        // A20: the split-wire topic, normalized (trim; empty = none).
        topic: octoscode_store::domains::approval::normalized_topic(requested.topic.as_deref()).map(str::to_owned),
        ..Default::default()
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
            let id = decided.approval_id.0.to_string();
            // A20: only a frame naming the record's exact origin and turn
            // settles it.
            if !resolves(&self.store, &id, &decided.session_id.0, decided.topic.as_deref(), &decided.turn_id.0.to_string()) {
                ::log::warn!("octoscode: approval/decided {id} for a foreign session/turn ignored");
                return;
            }
            self.store.domains.approval.settle(&id, decided.auto_resolved);
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
            let id = cancelled.approval_id.0.to_string();
            if !resolves(&self.store, &id, &cancelled.session_id.0, cancelled.topic.as_deref(), &cancelled.turn_id.0.to_string()) {
                ::log::warn!("octoscode: approval/cancelled {id} for a foreign session/turn ignored");
                return;
            }
            self.store.domains.approval.cancel(&id);
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
            if resolves(&self.store, &id, &auto.session_id.0, auto.topic.as_deref(), &auto.turn_id.0.to_string()) {
                self.store.domains.approval.settle(&id, true);
            } else {
                ::log::warn!("octoscode: approval/auto_resolved {id} for a foreign session/turn: the record stays");
            }
            // A6: the auto-resolve "toast" (parity row: "toast on
            // auto-resolve", 'Auto-approved' / 'Auto-denied') as a transcript
            // notice on the turn it unblocked. A policy decided without asking,
            // so no card ever showed: this line is the person's only record.
            // Deterministic id `approval:<id>`: a replayed (durable) event
            // updates the same row, never a second one.
            let approved = matches!(auto.decision, octos_core::ui_protocol::ApprovalDecision::Approve);
            let title = if approved { "Auto-approved" } else { "Auto-denied" };
            let body = format!("{} · matched the {} scope", auto.tool_name, auto.scope);
            self.store.domains.session.timeline.upsert_notice_data(
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
            // A20: the question's exact origin, as for an approval; a server
            // that advertised its methods without `user_question/respond` +
            // `user_question.v1` gets no card (`:264-276`).
            let Some(owner) = interaction_owner(&self.store, &e.session_id.0, e.topic.as_deref()) else {
                ::log::warn!(
                    "octoscode: user_question/requested {} for a foreign session/topic ({} / {:?}) rejected",
                    e.question_id.0,
                    e.session_id.0,
                    e.topic
                );
                return;
            };
            if !negotiated(&self.store, "user_question/respond", Some("user_question.v1")) {
                ::log::warn!("octoscode: user_question/requested without an advertised user_question/respond rejected");
                return;
            }
            self.store.domains.approval.set_question(PendingQuestion {
                question_id: e.question_id.0.to_string(),
                session_id: e.session_id.0.clone(),
                turn_id: e.turn_id.0.to_string(),
                title: e.title.clone(),
                body: e.body.clone(),
                questions: serde_json::to_value(&e.questions).unwrap_or(serde_json::Value::Null),
                topic: octoscode_store::domains::approval::normalized_topic(e.topic.as_deref()).map(str::to_owned),
                generation: self.store.domains.approval.generation(),
                unread: self.store.active_session().as_deref() != Some(owner.as_str()),
                seq: 0,
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
