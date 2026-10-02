//! A10 — the native peer manager (web `features/peers/peer-manager.ts`,
//! `peer-roster.ts`, `peer-row-view.ts`, `gather.ts`,
//! `session/session-peer-coordinator.ts`), over the store's roster
//! (`octoscode_store::domains::peer::PeerRow`).
//!
//! * **stage / open / close, authority-fenced** — `peer/staged` from the
//!   CONFIRMED master scope (session + profile) stages ONE row per identity
//!   (`<profile>:local:tui#peer-<slug>`, `peerIdentityForTopic`); a replay
//!   dedupes; a closed identity is tombstoned and never reopened by replay
//!   (`#stage` / `#close`). The open runs in the BACKGROUND — a raw
//!   `session/open` of the peer session on this connection (the web's pooled
//!   client) and ONE kickoff `turn/start` — never changing the active session.
//! * **the activity axis** (`observeSessionEvent`) is folded from the peer
//!   Session's OWN frames ([`session_event`] maps the notification; the
//!   `flow.rs` hook hands a tracked peer session's frames here instead of the
//!   master's timeline): turn started → live; approval / question requested →
//!   blocked with the REAL pending id + contents; decided / auto-resolved /
//!   cancelled → the pre-block activity; token-cost progress → tokens; turn
//!   completed → done/finished, turn error `interrupted` → stopped, any other
//!   error → failed.
//! * **the row view** — `Peer N · model` labels (never a slug), the action
//!   table, elapsed (`Ns` / `MmSSs` / `HhMMm`), `↓` tokens, the answer card.
//! * **gather** — `composeGatherPrompt` (the 64 KiB cap with equal UTF-8
//!   budgets).
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_store::domains::peer::{
    Activity, ApprovalDetail, Origin, Outcome, PeerRow, PeerSessionEvent, QuestionDetail, RequestDetail,
    RequestKind,
};
use octoscode_store::Store;
use serde_json::{json, Value};

// ---------------------------------------------------------------- identity

/// `peerIdentityForTopic` (`packages/client/src/peer-protocol.ts:115-123`).
pub fn identity_for_topic(profile_id: &str, topic: &str) -> String {
    format!("{profile_id}:local:tui#{topic}")
}

/// `peerKickoffPrompt` (`peer-manager.ts:45-47`, TUI store.rs).
pub fn kickoff_prompt(brief: &str, brief_path: &str) -> String {
    format!(
        "You are a peer agent. Your brief:\n\n{brief}\n\n(The durable copy of this brief is at {brief_path} — re-read it if your context is compacted.)"
    )
}

/// The per-member lens a fleet prepare (`n > 1`) gives each member
/// (`peer-manager.ts:212-218`).
pub fn fleet_member_brief(brief: &str, index: usize, n: usize) -> String {
    if n > 1 {
        format!(
            "{brief}\n\n(You are fleet member {} of {n} — peers were given this same brief; differentiate your angle.)",
            index + 1
        )
    } else {
        brief.to_owned()
    }
}

/// Wall-clock ms.
pub fn now_ms() -> u64 {
    octoscode_store::domains::peer::now_ms()
}

// ------------------------------------------------------- session events

fn s(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

/// Map ONE notification onto `(session_id, event)` for the peer-session fold
/// (`peerSessionEventFor`, `session-peer-coordinator.ts:139-240`). `None` for
/// a frame that carries no roster fact.
pub fn session_event(n: &UiNotification) -> Option<(String, PeerSessionEvent)> {
    use octos_core::ui_protocol::{PayloadV2, TurnTerminalOutcome};
    Some(match n {
        UiNotification::TurnStarted(e) => (
            e.session_id.0.clone(),
            PeerSessionEvent::TurnStarted { turn_id: Some(e.turn_id.0.to_string()) },
        ),
        UiNotification::TurnCompleted(e) => (
            e.session_id.0.clone(),
            PeerSessionEvent::TurnTerminal { outcome: Outcome::Finished, error: None },
        ),
        // J3: `interrupted` is Stopped; any other error is Failed — never
        // Finished.
        UiNotification::TurnError(e) => (
            e.session_id.0.clone(),
            PeerSessionEvent::TurnTerminal {
                outcome: if e.code == "interrupted" { Outcome::Stopped } else { Outcome::Failed },
                error: s(&e.message),
            },
        ),
        UiNotification::ApprovalRequested(e) => {
            let target = e
                .typed_details
                .as_ref()
                .and_then(|d| serde_json::to_value(d).ok())
                .and_then(|v| v.pointer("/command/command_line").and_then(Value::as_str).map(str::to_owned));
            (
                e.session_id.0.clone(),
                PeerSessionEvent::AttentionRequested {
                    request_id: Some(e.approval_id.0.to_string()),
                    kind: Some(RequestKind::Approval),
                    detail: Some(RequestDetail::Approval(ApprovalDetail {
                        tool_name: e.tool_name.clone(),
                        target,
                        scope: e.approval_kind.clone(),
                        title: s(&e.title),
                        body: s(&e.body),
                    })),
                },
            )
        }
        UiNotification::UserQuestionRequested(e) => {
            let q = e.questions.first();
            (
                e.session_id.0.clone(),
                PeerSessionEvent::AttentionRequested {
                    request_id: Some(e.question_id.0.to_string()),
                    kind: Some(RequestKind::Question),
                    detail: Some(RequestDetail::Question(QuestionDetail {
                        header: q.and_then(|q| s(&q.header)),
                        question: q.and_then(|q| s(&q.question)),
                        options: q
                            .map(|q| q.options.iter().map(|o| (o.label.clone(), s(&o.description))).collect())
                            .unwrap_or_default(),
                        multi_select: q.is_some_and(|q| q.multi_select),
                        allow_free_text: q.is_some_and(|q| q.allow_free_text),
                    })),
                },
            )
        }
        // A30: each resolution names the approval it settles, so a late or
        // duplicate one never clears a NEWER pending request (the store's
        // `AttentionResolvedFor`).
        UiNotification::ApprovalDecided(e) => (
            e.session_id.0.clone(),
            PeerSessionEvent::AttentionResolvedFor { request_id: e.approval_id.0.to_string() },
        ),
        UiNotification::ApprovalAutoResolved(e) => (
            e.session_id.0.clone(),
            PeerSessionEvent::AttentionResolvedFor { request_id: e.approval_id.0.to_string() },
        ),
        UiNotification::ApprovalCancelled(e) => (
            e.session_id.0.clone(),
            PeerSessionEvent::AttentionResolvedFor { request_id: e.approval_id.0.to_string() },
        ),
        UiNotification::ProgressUpdated(e) => {
            let meta = serde_json::to_value(&e.metadata).ok()?;
            if meta.get("kind").and_then(Value::as_str) != Some("token_cost_update") {
                return None;
            }
            let out = meta.pointer("/token_cost/output_tokens").and_then(Value::as_u64)?;
            (e.session_id.0.clone(), PeerSessionEvent::Usage { output_tokens: out })
        }
        UiNotification::EnvelopeV2(frame) => match &frame.envelope.payload {
            PayloadV2::TurnTerminal { outcome, error, .. } => (
                frame.session_id.0.clone(),
                PeerSessionEvent::TurnTerminal {
                    outcome: match outcome {
                        TurnTerminalOutcome::Completed => Outcome::Finished,
                        TurnTerminalOutcome::Interrupted => Outcome::Stopped,
                        _ => Outcome::Failed,
                    },
                    error: error.as_ref().map(|e| e.message.clone()),
                },
            ),
            _ => return None,
        },
        _ => return None,
    })
}

/// The `flow.rs` hook: a frame of a TRACKED peer session (a roster row's
/// identity that is not the active master session) folds into that row and
/// never reaches the master's timeline. Returns whether the frame was a peer
/// session's (owned — the caller drops it from the master path).
pub fn fold_frame(store: &Store, n: &UiNotification) -> bool {
    let Some(sid) = notification_session(n) else { return false };
    if store.active_session().as_deref() == Some(sid.as_str()) {
        return false;
    }
    if store.domains.peer.row(&sid).is_none() {
        return false;
    }
    if let Some((session, event)) = session_event(n) {
        let owned = store.domains.peer.observe_session_event(&session, &event, now_ms());
        if owned {
            makepad_widgets::log!("[octoscode] peer session {session}: {event:?}");
            makepad_widgets::SignalToUI::set_ui_signal();
        }
    }
    true
}

/// The drain-loop hook (`lib.rs`, beside the other screens' folds): a
/// `peer/staged` from the CONFIRMED master scope stages a row and runs its
/// background open (`fleet_driver::open_staged`); `peer/closed` closes it.
pub fn note_transport_event(conv: &Arc<crate::flow::Conversation>, evt: &octos_app_transport::TransportEvent) {
    use octos_app_transport::TransportEvent;
    let payload = match evt {
        TransportEvent::DurableNotification { payload, .. } | TransportEvent::EphemeralNotification { payload } => payload,
        _ => return,
    };
    match payload {
        UiNotification::PeerStaged(e) => {
            if let Some(req) = observe_staged(&conv.store, &conv.session_id(), &conv.profile(), e) {
                makepad_widgets::log!("[octoscode] peer staged: {} (opening in the background)", req.identity);
                let c = conv.clone();
                tokio::spawn(async move {
                    crate::screens::fleet_driver::open_staged(&c, req).await;
                    makepad_widgets::SignalToUI::set_ui_signal();
                });
            }
        }
        UiNotification::PeerClosed(e) => {
            if observe_closed(&conv.store, &conv.session_id(), &conv.profile(), e) {
                makepad_widgets::log!("[octoscode] peer closed: {}", e.slug);
            }
        }
        _ => {}
    }
}

/// The session id a notification is scoped to (the frames a peer session
/// sends; `None` for an unscoped frame).
pub fn notification_session(n: &UiNotification) -> Option<String> {
    Some(match n {
        UiNotification::TurnStarted(e) => e.session_id.0.clone(),
        UiNotification::TurnCompleted(e) => e.session_id.0.clone(),
        UiNotification::TurnError(e) => e.session_id.0.clone(),
        UiNotification::MessageDelta(e) => e.session_id.0.clone(),
        UiNotification::ReasoningDelta(e) => e.session_id.0.clone(),
        UiNotification::ToolStarted(e) => e.session_id.0.clone(),
        UiNotification::ToolProgress(e) => e.session_id.0.clone(),
        UiNotification::ToolCompleted(e) => e.session_id.0.clone(),
        UiNotification::ApprovalRequested(e) => e.session_id.0.clone(),
        UiNotification::ApprovalDecided(e) => e.session_id.0.clone(),
        UiNotification::ApprovalAutoResolved(e) => e.session_id.0.clone(),
        UiNotification::ApprovalCancelled(e) => e.session_id.0.clone(),
        UiNotification::UserQuestionRequested(e) => e.session_id.0.clone(),
        UiNotification::ProgressUpdated(e) => e.session_id.0.clone(),
        UiNotification::EnvelopeV2(e) => e.session_id.0.clone(),
        _ => return None,
    })
}

// ------------------------------------------------------- staged / closed

/// One background open the host must run (`PeerOpenRequest`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub identity: String,
    pub profile_id: String,
    pub slug: String,
    pub cwd: String,
    pub brief_path: String,
    /// The kickoff turn (minted ONCE per identity, reused on retry).
    pub turn_id: String,
    pub prompt: String,
}

/// `observeNotification` for `peer/staged` (`peer-manager.ts:280-300` +
/// `parsePeerNotification`): only the CONFIRMED master scope (its session AND
/// profile) stages; the topic must be `peer-<slug>`. Returns the open the
/// host runs, or `None` (foreign scope, invalid identity, replay, tombstone).
pub fn observe_staged(
    store: &Store,
    master_session: &str,
    profile_id: &str,
    e: &octos_core::ui_protocol::PeerStagedEvent,
) -> Option<OpenRequest> {
    if e.session_id.0 != master_session || e.profile_id != profile_id {
        return None;
    }
    if e.topic != format!("peer-{}", e.slug)
        || !octoscode_client::domains::external_driver::peer_slug_is_safe(&e.slug)
        || e.brief.is_empty()
    {
        return None;
    }
    stage(store, profile_id, &e.slug, &e.cwd, &e.brief_path, &e.brief, Origin::Staged, false)
}

/// `#stage` (`peer-manager.ts:514-597`): one row, one kickoff UUID.
#[allow(clippy::too_many_arguments)]
pub fn stage(
    store: &Store,
    profile_id: &str,
    slug: &str,
    cwd: &str,
    brief_path: &str,
    brief: &str,
    origin: Origin,
    authorize_reuse: bool,
) -> Option<OpenRequest> {
    let identity = identity_for_topic(profile_id, &format!("peer-{slug}"));
    let turn_id = octoscode_client::domains::external_driver::new_operation_id();
    let mut row = PeerRow::opening(&identity, slug, origin, &turn_id, now_ms());
    row.profile_id = profile_id.to_owned();
    row.cwd = cwd.to_owned();
    row.brief_path = brief_path.to_owned();
    row.brief = brief.to_owned();
    if !store.domains.peer.stage_row(row, authorize_reuse) {
        return None;
    }
    Some(OpenRequest {
        identity,
        profile_id: profile_id.to_owned(),
        slug: slug.to_owned(),
        cwd: cwd.to_owned(),
        brief_path: brief_path.to_owned(),
        turn_id,
        prompt: kickoff_prompt(brief, brief_path),
    })
}

/// `observeNotification` for `peer/closed`: the master scope only.
pub fn observe_closed(
    store: &Store,
    master_session: &str,
    profile_id: &str,
    e: &octos_core::ui_protocol::PeerClosedEvent,
) -> bool {
    if e.session_id.0 != master_session || e.profile_id != profile_id || e.topic != format!("peer-{}", e.slug) {
        return false;
    }
    let identity = identity_for_topic(profile_id, &e.topic);
    // A dispatched row was re-keyed to its adopted identity: close by slug.
    let key = store
        .domains
        .peer
        .row(&identity)
        .map(|r| r.identity)
        .or_else(|| store.domains.peer.row_by_slug(&e.slug).map(|r| r.identity))
        .unwrap_or(identity);
    store.domains.peer.close_row(&key)
}

/// The BACKGROUND open + kickoff (`session-peer-coordinator.ts` non-dispatch
/// path): `session/open` of the peer session on this connection (the active
/// session is untouched), then ONE `turn/start` with the kickoff prompt. The
/// row settles `started` (the kickoff was accepted) or `failed` (retryable) /
/// `unknown`.
pub async fn open_peer(conv: &crate::flow::Conversation, req: &OpenRequest) -> Result<(), String> {
    let store = &conv.store;
    let opened = conv
        .client()
        .request(
            "session/open",
            json!({ "session_id": req.identity, "profile_id": req.profile_id, "cwd": req.cwd }),
        )
        .await;
    if let Err(e) = opened {
        store
            .domains
            .peer
            .mark_not_started(&req.identity, false, "Peer session could not be opened; no kickoff was queued.");
        return Err(e.to_string());
    }
    if store.domains.peer.is_tombstoned(&req.identity) {
        return Err("closed before kickoff".into());
    }
    let started = conv
        .client()
        .request(
            "turn/start",
            json!({
                "session_id": req.identity,
                "turn_id": req.turn_id,
                "input": [{ "kind": "text", "text": req.prompt }],
            }),
        )
        .await;
    match started {
        Ok(_) => {
            store
                .domains
                .peer
                .mark_started(&req.identity, &req.identity, &req.slug, None, Some(&req.turn_id), None, now_ms());
            Ok(())
        }
        Err(e) => {
            store.domains.peer.mark_not_started(
                &req.identity,
                true,
                "The peer start could not be confirmed. Inspect the peer session before retrying.",
            );
            Err(e.to_string())
        }
    }
}

// ----------------------------------------------------------------- gather

/// `composeGatherPrompt` (`gather.ts:13-47`): equal UTF-8 result budgets so
/// the prompt fits 64 KiB; an unbounded scaffolding is refused.
pub fn compose_gather_prompt(peers: &[octoscode_client::domains::peer::PeerGatherEntry]) -> Result<String, String> {
    const MAX: usize = 64 * 1024;
    const NOTE: &str = "\n[…result truncated to fit the gather prompt cap]";
    let build = |budget: Option<usize>| {
        let mut out = String::from("Peer results gathered from the blackboard:\n");
        for p in peers {
            let brief: String = p.brief.chars().take(200).collect();
            out.push_str(&format!(
                "\n## peer {} ({})\nBrief: {brief}\n\n",
                p.slug,
                if p.result.is_some() { "done" } else { "no result yet" }
            ));
            let Some(result) = &p.result else {
                out.push_str("(still running — no result file yet)\n");
                continue;
            };
            match budget {
                Some(b) if result.len() > b => {
                    let mut end = b;
                    while end > 0 && !result.is_char_boundary(end) {
                        end -= 1;
                    }
                    out.push_str(&result[..end]);
                    out.push_str(NOTE);
                }
                _ => out.push_str(result),
            }
            out.push('\n');
        }
        out
    };
    let full = build(None);
    if full.len() <= MAX {
        return Ok(full);
    }
    let overhead = build(Some(0)).len();
    if overhead > MAX {
        return Err("Peer gather scaffolding exceeds the prompt limit".into());
    }
    let count = peers.iter().filter(|p| p.result.is_some()).count().max(1);
    Ok(build(Some((MAX - overhead) / count)))
}

// --------------------------------------------------------------- row view

/// `peerRowLabel` (`peer-row-view.ts:23-27`): never the slug.
pub fn row_label(index: usize, model: Option<&str>) -> String {
    match model {
        Some(m) if !m.is_empty() => format!("Peer {} · {m}", index + 1),
        _ => format!("Peer {}", index + 1),
    }
}

/// `formatElapsed` (`peer-row-view.ts:127-133`, TUI `format_short_duration`).
pub fn format_elapsed(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// `formatPeerTokens` (`peer-row-view.ts:135-149`).
pub fn format_tokens(n: u64) -> String {
    let trim = |v: f64| {
        if v.fract() == 0.0 {
            format!("{}", v as u64)
        } else {
            format!("{v:.1}")
        }
    };
    if n >= 1_000_000 {
        format!("↓ {}M", trim(n as f64 / 1_000_000.0))
    } else if n >= 1_000 {
        format!("↓ {}k", trim(n as f64 / 1_000.0))
    } else {
        format!("↓ {n}")
    }
}

/// The product row actions (`PeerRowAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    Approve,
    ApproveSession,
    Deny,
    Answer,
    Steer,
    Stop,
}

/// `peerRowActions` (`peer-row-view.ts:37-52`): a row without an accepted
/// dispatch operation id is unaddressable — no affordance.
pub fn row_actions(row: &PeerRow) -> Vec<RowAction> {
    let addressable = row.operation_id.as_deref().is_some_and(|o| !o.is_empty());
    if !addressable {
        return Vec::new();
    }
    match row.activity {
        Activity::Blocked => match row.request_kind {
            Some(RequestKind::Approval) => {
                vec![RowAction::Approve, RowAction::ApproveSession, RowAction::Deny, RowAction::Stop]
            }
            Some(RequestKind::Question) => vec![RowAction::Answer, RowAction::Stop],
            None => vec![RowAction::Stop],
        },
        Activity::Live | Activity::Idle => vec![RowAction::Steer, RowAction::Stop],
        Activity::Done => Vec::new(),
    }
}

/// `peerAnswerRequest` (`peer-row-view.ts:90-120`): the Answer card's shape,
/// NULL unless question-blocked with a REAL id and the stamped detail.
pub fn answer_request(row: &PeerRow) -> Option<Value> {
    if row.activity != Activity::Blocked || row.request_kind != Some(RequestKind::Question) {
        return None;
    }
    let qid = row.request_id.as_deref().filter(|i| !i.is_empty())?;
    let Some(RequestDetail::Question(q)) = &row.request_detail else { return None };
    let title = q.header.clone().unwrap_or_else(|| row.slug.clone());
    let body = q.question.clone().unwrap_or_default();
    Some(json!({
        "sessionId": row.identity,
        "questionId": qid,
        "turnId": row.turn_id,
        "title": title,
        "body": body,
        "questions": [{
            "header": q.header.clone().unwrap_or_else(|| title.clone()),
            "question": q.question.clone().unwrap_or_else(|| body.clone()),
            "options": q.options.iter().map(|(l, d)| json!({"label": l, "description": d})).collect::<Vec<_>>(),
            "multiSelect": q.multi_select,
            "allowFreeText": q.allow_free_text,
        }],
    }))
}

/// `summarizeRoster` (`peer-manager.ts:763-780`): the four buckets sum to
/// `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RosterCounts {
    pub total: usize,
    pub live: usize,
    pub blocked: usize,
    pub done: usize,
    pub idle: usize,
}

pub fn summarize(rows: &[PeerRow]) -> RosterCounts {
    let mut c = RosterCounts { total: rows.len(), live: 0, blocked: 0, done: 0, idle: 0 };
    for r in rows {
        match r.activity {
            Activity::Live => c.live += 1,
            Activity::Blocked => c.blocked += 1,
            Activity::Done => c.done += 1,
            Activity::Idle => c.idle += 1,
        }
    }
    c
}

/// `fleetLanded`: `done` rows only.
pub fn fleet_landed(rows: &[PeerRow]) -> (usize, usize) {
    (rows.iter().filter(|r| r.activity == Activity::Done).count(), rows.len())
}

/// `formatPeerDockPill` (`peer-row-view.ts:168-180`).
pub fn dock_pill(rows: &[PeerRow]) -> String {
    let c = summarize(rows);
    let (landed, total) = fleet_landed(rows);
    let mut parts = vec![format!("{}", c.total), format!("{} live", c.live), format!("{landed}/{total} landed")];
    if c.blocked > 0 {
        parts.push(format!("{} blocked", c.blocked));
    }
    if c.done > 0 {
        parts.push(format!("{} done", c.done));
    }
    parts.join(" · ")
}

// ------------------------------------------------------- the action ids

/// The legacy control ids this module owns (the dock's row affordances).
pub fn owns(action: &str) -> bool {
    matches!(action, "peer.approve" | "peer.deny" | "peer.answer" | "peer.stop" | "peer.roster")
}

/// The legacy control path, routed through the ONE production control chain
/// (`screens::fleet_driver::row_control`): `value` names the row by slug.
pub async fn perform(
    conv: &crate::flow::Conversation,
    action: &str,
    store: &Store,
    value: Option<&str>,
) -> Result<String, String> {
    if action == "peer.roster" {
        let rows = store.domains.peer.rows();
        let c = summarize(&rows);
        let (landed, total) = fleet_landed(&rows);
        return Ok(format!(
            "{} peers — {} live, {} blocked, {} done, {} idle; {landed}/{total} landed",
            c.total, c.live, c.blocked, c.done, c.idle
        ));
    }
    let slug = value.unwrap_or_default();
    let row = store
        .domains
        .peer
        .row_by_slug(slug)
        .ok_or_else(|| format!("No peer named {slug:?}."))?;
    let act = match action {
        "peer.approve" => RowAction::Approve,
        "peer.deny" => RowAction::Deny,
        "peer.answer" => RowAction::Answer,
        "peer.stop" => RowAction::Stop,
        other => return Err(format!("peers: unhandled action {other:?}")),
    };
    if !row_actions(&row).contains(&act) {
        return Err(format!("{act:?} is not available for this peer right now."));
    }
    crate::screens::fleet_driver::row_control(conv, &row.identity, act, "").await
}

/// Keep `Arc` in scope for the host's spawn sites.
pub type SharedStore = Arc<Store>;

/// A7 — `peerIdentityForTopic` (`packages/client/src/peer-protocol.ts:115-123`):
/// a native peer's own Session id is `<profile>:local:tui#<topic>`, valid only
/// for a profile without `:`/`#`/whitespace and a `peer-<slug>` topic.
pub fn peer_identity_for_topic(profile_id: &str, topic: &str) -> Option<String> {
    let profile_ok = !profile_id.trim().is_empty()
        && !profile_id.chars().any(|c| c == ':' || c == '#' || c.is_whitespace());
    let slug = topic.strip_prefix("peer-")?;
    let slug_ok = slug.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    (profile_ok && slug_ok).then(|| format!("{profile_id}:local:tui#{topic}"))
}

/// A7 — `peerReadonlySlug` (`apps/web/src/features/composer/peer-readonly.ts:
/// 23-32`): the slug of the OPENED peer whose native Session id EQUALS
/// `session_id` — exact membership in the roster's identity set, never a
/// `peer-` prefix match (which would false-positive on an ordinary Session
/// whose topic merely starts with `peer-`). A closed peer no longer counts.
pub fn readonly_slug(store: &Store, session_id: &str) -> Option<String> {
    if session_id.is_empty() {
        return None;
    }
    store.domains.peer.list().into_iter().find_map(|p| {
        if p.closed {
            return None;
        }
        let identity = peer_identity_for_topic(p.profile_id.as_deref()?, p.topic.as_deref()?)?;
        (identity == session_id).then(|| p.name.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A7 — the store's roster as `peer/staged` folds it (`observe_staged`).
    fn a7_roster() -> Store {
        let store = Store::new();
        for slug in ["review", "audit"] {
            store.domains.peer.observe_staged(octoscode_store::domains::peer::Peer {
                topic: Some(format!("peer-{slug}")),
                profile_id: Some("dev".into()),
                origin_session_id: Some("dev:main".into()),
                ..octoscode_store::domains::peer::Peer::named(slug)
            });
        }
        store
    }

    // peer-readonly.test.ts:14 "names the peer whose native session id is in
    // the roster"
    #[test]
    fn the_read_only_row_names_the_peer_whose_session_is_in_the_roster() {
        let store = a7_roster();
        assert_eq!(readonly_slug(&store, "dev:local:tui#peer-audit").as_deref(), Some("audit"));
        assert_eq!(readonly_slug(&store, "dev:local:tui#peer-review").as_deref(), Some("review"));
    }

    // :19 "returns null for an ordinary session id, an empty roster, or no
    // focus" + a closed peer no longer counts
    #[test]
    fn no_row_for_an_ordinary_session_an_empty_roster_or_no_focus() {
        let store = a7_roster();
        assert_eq!(readonly_slug(&store, "dev:local:tui#ordinary"), None);
        assert_eq!(readonly_slug(&Store::new(), "dev:local:tui#peer-review"), None);
        assert_eq!(readonly_slug(&store, ""), None);
        store.domains.peer.mark_closed("audit");
        assert_eq!(readonly_slug(&store, "dev:local:tui#peer-audit"), None, "a closed peer is not open");
    }

    // :27 "never matches a merely peer-prefixed identity outside the exact
    // roster"
    #[test]
    fn a_peer_prefixed_identity_outside_the_roster_never_matches() {
        let store = a7_roster();
        assert_eq!(readonly_slug(&store, "dev:local:tui#peer-review-evil"), None);
        assert_eq!(peer_identity_for_topic("dev", "review"), None, "a topic must be peer-<slug>");
        assert_eq!(peer_identity_for_topic("a:b", "peer-x"), None, "a profile carries no ':'");
    }

    #[test]
    fn identities_prompts_and_formats_are_the_webs() {
        assert_eq!(identity_for_topic("dsflash", "peer-a"), "dsflash:local:tui#peer-a");
        assert!(kickoff_prompt("Do X", "/p/brief.md").starts_with("You are a peer agent. Your brief:\n\nDo X\n\n"));
        assert!(fleet_member_brief("b", 1, 3).ends_with("(You are fleet member 2 of 3 — peers were given this same brief; differentiate your angle.)"));
        assert_eq!(fleet_member_brief("b", 0, 1), "b");
        assert_eq!(format_elapsed(42_000), "42s");
        assert_eq!(format_elapsed(90_000), "1m30s");
        assert_eq!(format_elapsed(7_500_000), "2h05m");
        assert_eq!(format_tokens(950), "↓ 950");
        assert_eq!(format_tokens(1_200), "↓ 1.2k");
        assert_eq!(format_tokens(3_000_000), "↓ 3M");
        assert_eq!(row_label(1, Some("gpt-5.4")), "Peer 2 · gpt-5.4");
        assert_eq!(row_label(0, None), "Peer 1");
    }

    #[test]
    fn row_actions_follow_the_activity_and_need_an_operation_id() {
        let mut r = PeerRow::opening("m#peer-a", "a", Origin::Dispatch, "t", 1);
        assert!(row_actions(&r).is_empty(), "unaddressable");
        r.operation_id = Some("op".into());
        r.activity = Activity::Live;
        assert_eq!(row_actions(&r), vec![RowAction::Steer, RowAction::Stop]);
        r.activity = Activity::Blocked;
        r.request_kind = Some(RequestKind::Approval);
        assert_eq!(row_actions(&r), vec![RowAction::Approve, RowAction::ApproveSession, RowAction::Deny, RowAction::Stop]);
        r.request_kind = Some(RequestKind::Question);
        r.request_id = Some("q1".into());
        r.request_detail = Some(RequestDetail::Question(QuestionDetail {
            header: Some("Pick".into()),
            question: Some("Which?".into()),
            options: vec![("A".into(), None)],
            ..Default::default()
        }));
        assert_eq!(row_actions(&r), vec![RowAction::Answer, RowAction::Stop]);
        let card = answer_request(&r).expect("the answer card");
        assert_eq!(card["questionId"], "q1");
        assert_eq!(card["questions"][0]["options"][0]["label"], "A");
        r.activity = Activity::Done;
        assert!(row_actions(&r).is_empty());
    }

    #[test]
    fn the_gather_prompt_caps_at_64k_with_equal_budgets() {
        use octoscode_client::domains::peer::PeerGatherEntry;
        let entry = |slug: &str, result: Option<String>| PeerGatherEntry {
            slug: slug.into(),
            topic: format!("peer-{slug}"),
            name: None,
            brief: "brief".into(),
            brief_truncated: false,
            result,
            result_truncated: false,
            result_updated_unix: None,
            has_worktree: false,
            closed: false,
        };
        let small = compose_gather_prompt(&[entry("a", Some("done!".into())), entry("b", None)]).unwrap();
        assert!(small.starts_with("Peer results gathered from the blackboard:\n"));
        assert!(small.contains("## peer a (done)") && small.contains("(still running — no result file yet)"));
        let big = compose_gather_prompt(&[entry("a", Some("x".repeat(50_000))), entry("b", Some("y".repeat(50_000)))])
            .unwrap();
        assert!(big.len() <= 64 * 1024, "{}", big.len());
        assert_eq!(big.matches("[…result truncated to fit the gather prompt cap]").count(), 2);
    }
}
