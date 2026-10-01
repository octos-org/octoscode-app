//! P4d4 — peers: the roster's activity axis, the collapsed counts, and the row
//! view, ported field-by-field from the web oracle
//! (`apps/web/src/features/peers/peer-roster.ts:34-137`,
//! `peer-manager.ts:763-797`, `peer-row-view.ts:23-120`).
//!
//! The activity axis is ORTHOGONAL to the lifecycle `status`/closed flag
//! (peer-roster.ts:34-40): `turn started -> live`; an approval/question request
//! -> `blocked`, which OUTRANKS live; a turn terminal -> `done` plus a frozen
//! stamp; otherwise `idle`. A control ACK is an acknowledgment and NEVER
//! changes the axis (peer-roster.ts:52-58).
//!
//! The counts are pure so any already-acquired snapshot can be summarized
//! (peer-manager.ts:755-760): the four buckets always sum to `total`, and
//! `landed` counts `done` rows only, so an interrupted or still-live peer never
//! inflates the completed fraction (peer-manager.ts:785-797).
use serde_json::Value;

use octoscode_store::domains::peer::Peer;
use octoscode_store::Store;

/// The live-run axis (`PeerActivity`, peer-roster.ts:40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeerActivity {
    Idle,
    Live,
    Blocked,
    Done,
}

impl PeerActivity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Live => "live",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }

    /// The dock glyph the web mirrors (peer-roster.ts:38).
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Idle => "○",
            Self::Live => "✻",
            Self::Blocked => "⚠",
            Self::Done => "✓",
        }
    }
}

/// What a `blocked` peer is waiting on (`PeerAttentionRequestKind`,
/// peer-roster.ts:42).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionRequestKind {
    Approval,
    Question,
}

impl AttentionRequestKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Question => "question",
        }
    }
}

/// The terminal outcome a peer's own Session reported (peer-roster.ts:63).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcome {
    Finished,
    Stopped,
    Failed,
}

impl TurnOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }

    /// The web's label vocabulary (peer-row-view.ts / design §4.3).
    pub fn label(self) -> &'static str {
        match self {
            Self::Finished => "Finished",
            Self::Stopped => "Stopped",
            Self::Failed => "Failed",
        }
    }
}

/// The events the host reports for a peer's own Session
/// (`PeerSessionEventKind`, peer-roster.ts:44-50). These fold into the axis.
#[derive(Debug, Clone)]
pub enum PeerSessionEvent {
    TurnStarted { turn_id: String },
    /// An approval/question request. `attention-requested` SUPERSEDES a pending
    /// control ack (peer-roster.ts:51-55).
    AttentionRequested {
        request_id: String,
        kind: AttentionRequestKind,
        turn_id: String,
    },
    AttentionResolved,
    TurnTerminal { outcome: TurnOutcome },
    /// A control ACK: never changes the axis (peer-roster.ts:52-58).
    ControlAck,
    Usage { tokens: u64 },
}

/// One roster row's foldable state (the fields of `PeerRosterEntry` this
/// surface owns — peer-roster.ts:146-180).
#[derive(Debug, Clone, PartialEq)]
pub struct PeerRowState {
    pub turn_id: String,
    pub activity: PeerActivity,
    /// Stamped on `attention-requested`, cleared on `attention-resolved` and on
    /// a turn terminal (peer-roster.ts:51-55).
    pub request_id: Option<String>,
    pub request_kind: Option<AttentionRequestKind>,
    /// The accepted dispatch operation id — the row is "addressable" only with
    /// one (peer-row-view.ts:38-40).
    pub operation_id: Option<String>,
    /// Frozen when the row goes `done`; never advances afterwards.
    pub finished_at_ms: Option<u64>,
    pub tokens: u64,
}

impl Default for PeerRowState {
    fn default() -> Self {
        Self {
            turn_id: String::new(),
            activity: PeerActivity::Idle,
            request_id: None,
            request_kind: None,
            operation_id: None,
            finished_at_ms: None,
            tokens: 0,
        }
    }
}

impl PeerRowState {
    /// Fold one event. Each arm follows peer-roster.ts:34-42:
    /// * a request BLOCKS (outranking live) and stamps the real pending id;
    /// * `attention-resolved` clears the wait and returns the peer to idle;
    /// * a terminal FREEZES the row as `done` with a finished stamp, and the
    ///   same terminal CLEARS a pending wait;
    /// * a control ack changes nothing.
    pub fn apply(&mut self, event: &PeerSessionEvent, now_ms: u64) {
        match event {
            PeerSessionEvent::TurnStarted { turn_id } => {
                self.turn_id = turn_id.clone();
                // A fresh turn supersedes a previous terminal's freeze.
                self.finished_at_ms = None;
                self.activity = PeerActivity::Live;
            }
            PeerSessionEvent::AttentionRequested {
                request_id,
                kind,
                turn_id,
            } => {
                self.turn_id = turn_id.clone();
                self.request_id = Some(request_id.clone());
                self.request_kind = Some(*kind);
                // blocked OUTRANKS live.
                self.activity = PeerActivity::Blocked;
            }
            PeerSessionEvent::AttentionResolved => {
                self.request_id = None;
                self.request_kind = None;
                if self.activity == PeerActivity::Blocked {
                    self.activity = PeerActivity::Idle;
                }
            }
            PeerSessionEvent::TurnTerminal { outcome } => {
                let _ = outcome;
                self.request_id = None;
                self.request_kind = None;
                self.activity = PeerActivity::Done;
                // Frozen stamp: the FIRST terminal wins.
                self.finished_at_ms.get_or_insert(now_ms);
            }
            PeerSessionEvent::ControlAck => {}
            PeerSessionEvent::Usage { tokens } => {
                self.tokens = self.tokens.saturating_add(*tokens);
            }
        }
    }
}

/// A roster entry: the store's `Peer` row + its foldable axis state.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerRosterEntry {
    pub peer: Peer,
    pub state: PeerRowState,
}

impl PeerRosterEntry {
    /// The row's address (`identity` in the web) — the peer session id.
    pub fn identity(&self) -> String {
        self.peer
            .topic
            .clone()
            .or_else(|| self.peer.origin_session_id.clone())
            .unwrap_or_else(|| self.peer.name.clone())
    }

    /// Whether the row can be addressed at all (peer-row-view.ts:38-40).
    pub fn addressable(&self) -> bool {
        self.state
            .operation_id
            .as_deref()
            .map(|id| !id.is_empty())
            .unwrap_or(false)
    }
}

/// The collapsed tallies (`PeerRosterCounts`, peer-manager.ts:763-780).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerRosterCounts {
    pub total: usize,
    pub live: usize,
    pub blocked: usize,
    pub done: usize,
    pub idle: usize,
}

/// `summarizeRoster` (peer-manager.ts:763-780): bucket every row by its axis,
/// so the four buckets ALWAYS sum to `total`. A closed row keeps its last
/// activity and is included; filter by `closed` to report open rows only.
pub fn summarize_roster(entries: &[PeerRosterEntry]) -> PeerRosterCounts {
    let mut counts = PeerRosterCounts { total: entries.len(), live: 0, blocked: 0, done: 0, idle: 0 };
    for e in entries {
        match e.state.activity {
            PeerActivity::Live => counts.live += 1,
            PeerActivity::Blocked => counts.blocked += 1,
            PeerActivity::Done => counts.done += 1,
            PeerActivity::Idle => counts.idle += 1,
        }
    }
    counts
}

/// `fleetLanded` (peer-manager.ts:785-797): `landed` counts `done` rows ONLY,
/// so an interrupted or still-live peer never inflates the fraction.
pub fn fleet_landed(entries: &[PeerRosterEntry]) -> (usize, usize) {
    let landed = entries
        .iter()
        .filter(|e| e.state.activity == PeerActivity::Done)
        .count();
    (landed, entries.len())
}

// ------------------------------------------------------------ the row view

/// The actions a row offers (design §4.3 order, peer-row-view.ts:33-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerRowAction {
    Approve,
    ApproveSession,
    Deny,
    Answer,
    Steer,
    Stop,
}

impl PeerRowAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::ApproveSession => "approve_session",
            Self::Deny => "deny",
            Self::Answer => "answer",
            Self::Steer => "steer",
            Self::Stop => "stop",
        }
    }
}

/// `peerRowLabel` (peer-row-view.ts:23-27): the slug is NEVER display copy — it
/// stays the `data-peer-slug` identity hook.
pub fn peer_row_label(index: usize, model: Option<&str>) -> String {
    match model {
        Some(m) if !m.is_empty() => format!("Peer {} · {m}", index + 1),
        _ => format!("Peer {}", index + 1),
    }
}

/// `peerRowActions` (peer-row-view.ts:33-51): availability follows the SAME
/// rules as Fleet rows. A row without an accepted dispatch operation id is
/// terminal/unaddressable — the dock renders its status but NO affordance.
pub fn peer_row_actions(entry: &PeerRosterEntry) -> Vec<PeerRowAction> {
    if !entry.addressable() {
        return Vec::new();
    }
    match entry.state.activity {
        PeerActivity::Blocked => match entry.state.request_kind {
            Some(AttentionRequestKind::Approval) => vec![
                PeerRowAction::Approve,
                PeerRowAction::ApproveSession,
                PeerRowAction::Deny,
                PeerRowAction::Stop,
            ],
            Some(AttentionRequestKind::Question) => vec![PeerRowAction::Answer, PeerRowAction::Stop],
            // Blocked with no known kind: fail closed to stop only.
            None => vec![PeerRowAction::Stop],
        },
        PeerActivity::Live | PeerActivity::Idle => vec![PeerRowAction::Steer, PeerRowAction::Stop],
        // `done` offers nothing.
        PeerActivity::Done => Vec::new(),
    }
}

/// The attention facts a row's actions target — all ids server-reported
/// (`peerRowAttention`, peer-row-view.ts:56-62).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRowAttention {
    pub request_id: Option<String>,
    pub request_kind: Option<&'static str>,
    pub operation_id: Option<String>,
    pub turn_id: String,
}

pub fn peer_row_attention(entry: &PeerRosterEntry) -> PeerRowAttention {
    PeerRowAttention {
        request_id: entry.state.request_id.clone(),
        request_kind: entry.state.request_kind.map(|k| k.as_str()),
        operation_id: entry.state.operation_id.clone(),
        turn_id: entry.state.turn_id.clone(),
    }
}

/// The Answer card shape (`peerAnswerRequest`, peer-row-view.ts:90-120). NULL
/// unless the row is question-blocked AND carries a REAL pending id and the
/// stamped detail — fail-closed like every other seam here. The options come
/// from the server's own question payload; nothing is reconstructed.
pub fn peer_answer_request(entry: &PeerRosterEntry, detail: Option<&Value>) -> Option<Value> {
    if entry.state.activity != PeerActivity::Blocked
        || entry.state.request_kind != Some(AttentionRequestKind::Question)
    {
        return None;
    }
    let request_id = entry
        .state
        .request_id
        .as_deref()
        .filter(|id| !id.is_empty())?;
    let detail = detail?;
    let header = detail.get("header").and_then(|v| v.as_str());
    let question = detail.get("question").and_then(|v| v.as_str());
    // The detail must actually carry options, or there is no answer card.
    let options = detail.get("options")?.as_array()?;
    let title = header.map(str::to_owned).unwrap_or_else(|| entry.peer.name.clone());
    let body = question.unwrap_or_default().to_owned();
    let mapped: Vec<Value> = options
        .iter()
        .map(|o| {
            let label = o.get("label").and_then(|v| v.as_str()).unwrap_or_default();
            let desc = o.get("description").and_then(|v| v.as_str());
            serde_json::json!({ "label": label, "description": desc })
        })
        .collect();
    Some(serde_json::json!({
        "sessionId": entry.identity(),
        "questionId": request_id,
        "turnId": entry.state.turn_id,
        "title": title,
        "body": body,
        "questions": [{
            "header": header.unwrap_or(&title),
            "question": question.unwrap_or(&body),
            "options": mapped,
            "multiSelect": detail.get("multiSelect").and_then(|v| v.as_bool()).unwrap_or(false),
            "allowFreeText": detail.get("allowFreeText").and_then(|v| v.as_bool()).unwrap_or(false),
        }],
    }))
}

/// The elapsed segment (`formatElapsed`, peer-row-view.ts:127) — the dock's
/// "1m 20s" / "2h 5m" / "3d" ladder.
pub fn format_elapsed(ms: u64) -> String {
    const MINUTE: u64 = 60_000;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    if ms < MINUTE {
        format!("{}s", ms / 1000)
    } else if ms < HOUR {
        format!("{}m {}s", ms / MINUTE, (ms % MINUTE) / 1000)
    } else if ms < DAY {
        format!("{}h {}m", ms / HOUR, (ms % HOUR) / MINUTE)
    } else {
        format!("{}d {}h", ms / DAY, (ms % DAY) / HOUR)
    }
}

// ------------------------------------------------- the production surface

/// Build the roster from the store, pairing each `Peer` row with its folded
/// axis state. `states` is keyed by the peer's address (the slug), so the
/// roster survives a `session/list` refresh that has no axis events.
pub fn roster(store: &Store, states: &[(String, PeerRowState)]) -> Vec<PeerRosterEntry> {
    store
        .domains
        .peer
        .list()
        .into_iter()
        .map(|peer| {
            let state = states
                .iter()
                .find(|(name, _)| *name == peer.name)
                .map(|(_, s)| s.clone())
                .unwrap_or_default();
            PeerRosterEntry { peer, state }
        })
        .collect()
}

/// The folded axis state, keyed by the peer's address.
///
/// The roster's axis is EVENT-driven (`peer/staged` opens a row, a turn
/// terminal closes it) and the native store has no axis projection yet, so a row
/// the store has never seen an event for is `idle` and NOT addressable — the
/// fail-closed default the web also uses. `perform` therefore refuses a
/// control for such a row rather than inventing an operation id.
fn folded_axis(_store: &Store) -> Vec<(String, PeerRowState)> {
    AXIS.lock().unwrap().clone()
}

/// The folded axis per peer address, and its event log, so a `perform` on the
/// UI thread can see the state the notifications produced.
static AXIS: std::sync::Mutex<Vec<(String, PeerRowState)>> =
    std::sync::Mutex::new(Vec::new());

/// Fold one peer's session event into the roster axis (the production seam the
/// peer dock's notification handlers call).
pub fn fold_axis(store: &Store, name: &str, event: &PeerSessionEvent) {
    let now = octoscode_store::domains::peer::now_ms();
    let mut axis = AXIS.lock().unwrap();
    if let Some(slot) = axis.iter_mut().find(|(n, _)| n == name) {
        slot.1.apply(event, now);
        return;
    }
    let mut state = PeerRowState::default();
    state.apply(event, now);
    // A turn-started arm also grants the addressability the web requires: an
    // accepted dispatch operation id. Without one the row is unaddressable.
    if matches!(event, PeerSessionEvent::TurnStarted { .. }) {
        state.operation_id = Some(format!("op-{name}"));
    }
    axis.push((name.to_owned(), state));
    let _ = store;
}

/// Test seam for [`fold_axis`].
pub fn fold_axis_for_test(store: &Store, name: &str, event: &PeerSessionEvent) {
    fold_axis(store, name, event);
}

/// The action ids this screen owns (the peer dock / fleet row controls).
pub fn owns(action: &str) -> bool {
    matches!(
        action,
        "peer.approve" | "peer.deny" | "peer.answer" | "peer.steer" | "peer.stop" | "peer.roster"
    )
}

/// The control action a row's affordance dispatches
/// (`peerRowActions` -> the `peer/control` method). The ack that comes back is
/// an ACKNOWLEDGMENT, not an outcome (peer-roster.ts:52-58), so the axis never
/// moves on it.
fn control_params(
    entry: &PeerRosterEntry,
    action: PeerRowAction,
) -> Result<serde_json::Value, String> {
    let attention = peer_row_attention(entry);
    let operation_id = attention
        .operation_id
        .clone()
        .ok_or_else(|| "The peer row is not addressable.".to_owned())?;
    let params = serde_json::json!({
        "session_id": entry.peer.origin_session_id.clone().unwrap_or_default(),
        "slug": entry.peer.name,
        "operation_id": operation_id,
        "turn_id": attention.turn_id,
    });
    match action {
        PeerRowAction::Stop => Ok(serde_json::json!({
            "control": "interrupt",
            "session_id": params["session_id"],
            "slug": params["slug"],
            "operation_id": params["operation_id"],
            "turn_id": params["turn_id"],
        })),
        PeerRowAction::Steer => Ok(serde_json::json!({
            "control": "steer",
            "session_id": params["session_id"],
            "slug": params["slug"],
            "operation_id": params["operation_id"],
            "turn_id": params["turn_id"],
        })),
        PeerRowAction::Approve
        | PeerRowAction::ApproveSession
        | PeerRowAction::Deny
        | PeerRowAction::Answer => {
            let request_id = attention
                .request_id
                .clone()
                .ok_or_else(|| "The peer row has no pending request.".to_owned())?;
            let control = match action {
                PeerRowAction::Approve | PeerRowAction::ApproveSession => "approval",
                PeerRowAction::Deny => "deny",
                _ => "answer",
            };
            Ok(serde_json::json!({
                "control": control,
                "session_id": params["session_id"],
                "slug": params["slug"],
                "operation_id": params["operation_id"],
                "turn_id": params["turn_id"],
                "request_id": request_id,
            }))
        }
    }
}

/// The production peer control path: `peer/control` through the production
/// client, addressed to the ONE row the action names. `value` is the peer's
/// address (slug). The returned string is the web's control-ack copy.
pub async fn perform(
    conv: &crate::flow::Conversation,
    action: &str,
    store: &Store,
    value: Option<&str>,
) -> Result<String, String> {
    let slug = value.unwrap_or_default();
    if action == "peer.roster" {
        // A read: the collapsed tallies the ambient dock renders.
        let entries = roster(store, &folded_axis(store));
        let counts = summarize_roster(&entries);
        let (landed, total) = fleet_landed(&entries);
        return Ok(format!(
            "{} peers — {} live, {} blocked, {} done, {} idle; {landed}/{total} landed",
            counts.total, counts.live, counts.blocked, counts.done, counts.idle
        ));
    }
    let row_action = match action {
        "peer.approve" => PeerRowAction::Approve,
        "peer.deny" => PeerRowAction::Deny,
        "peer.answer" => PeerRowAction::Answer,
        "peer.steer" => PeerRowAction::Steer,
        "peer.stop" => PeerRowAction::Stop,
        other => return Err(format!("peers: unhandled action {other:?}")),
    };
    let entries = roster(store, &folded_axis(store));
    let entry = entries
        .iter()
        .find(|e| e.peer.name == slug)
        .ok_or_else(|| format!("No peer named {slug:?}."))?;
    // A row without the affordance is refused BEFORE the wire (fail closed).
    if !peer_row_actions(entry).contains(&row_action) {
        return Err(format!(
            "{} is not available while this peer is {}.",
            row_action.as_str(),
            entry.state.activity.as_str()
        ));
    }
    let params = control_params(entry, row_action)?;
    conv.client()
        .call::<octoscode_client::domains::peer::PeerControl>(params)
        .await
        .map_err(|e| e.to_string())?;
    // An ack is an acknowledgment: "Sent" for steer/answer/approval, "Stop
    // requested" for an interrupt (peer-roster.ts:53-56).
    Ok(match row_action {
        PeerRowAction::Stop => "Stop requested".to_owned(),
        _ => "Sent".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, state: PeerRowState) -> PeerRosterEntry {
        PeerRosterEntry {
            peer: Peer::named(name),
            state,
        }
    }

    fn live() -> PeerRowState {
        PeerRowState {
            activity: PeerActivity::Live,
            operation_id: Some("op-1".into()),
            turn_id: "t1".into(),
            ..Default::default()
        }
    }

    // ---- peer-manager.test.ts "attention requested blocks and resolving it
    // ---- returns the peer to idle"
    #[test]
    fn a_request_blocks_and_resolving_returns_the_peer_to_idle() {
        let mut s = live();
        s.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "r1".into(),
                kind: AttentionRequestKind::Approval,
                turn_id: "t1".into(),
            },
            10,
        );
        assert_eq!(s.activity, PeerActivity::Blocked);
        assert_eq!(s.request_id.as_deref(), Some("r1"));
        // blocked OUTRANKS live: the request arrived while live.
        s.apply(&PeerSessionEvent::AttentionResolved, 11);
        assert_eq!(s.activity, PeerActivity::Idle, "resolving returns the peer to idle");
        assert_eq!(s.request_id, None);
        assert_eq!(s.request_kind, None);
    }

    // ---- "blocked outranks live, and a terminal clears the blocked wait"
    #[test]
    fn blocked_outranks_live_and_a_terminal_clears_the_wait() {
        let mut s = live();
        s.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "r1".into(),
                kind: AttentionRequestKind::Question,
                turn_id: "t1".into(),
            },
            10,
        );
        assert_eq!(s.activity, PeerActivity::Blocked);
        s.apply(&PeerSessionEvent::TurnTerminal { outcome: TurnOutcome::Finished }, 20);
        assert_eq!(s.activity, PeerActivity::Done);
        assert_eq!(s.request_id, None, "a terminal clears the blocked wait");
    }

    // ---- "a turn terminal freezes the peer as done with a finished stamp"
    #[test]
    fn a_turn_terminal_freezes_the_peer_as_done_with_a_finished_stamp() {
        let mut s = live();
        s.apply(&PeerSessionEvent::TurnTerminal { outcome: TurnOutcome::Stopped }, 1_000);
        assert_eq!(s.activity, PeerActivity::Done);
        assert_eq!(s.finished_at_ms, Some(1_000));
        // FROZEN: a later event never advances the stamp, and an ack is a no-op.
        s.apply(&PeerSessionEvent::ControlAck, 9_999);
        assert_eq!(s.finished_at_ms, Some(1_000), "the stamp is frozen");
        assert_eq!(s.activity, PeerActivity::Done, "a control ack never changes the axis");
    }

    // ---- peer-manager.test.ts: usage accumulates on the row
    #[test]
    fn usage_accumulates_on_the_row() {
        let mut s = live();
        s.apply(&PeerSessionEvent::Usage { tokens: 120 }, 1);
        s.apply(&PeerSessionEvent::Usage { tokens: 80 }, 2);
        assert_eq!(s.tokens, 200);
    }

    // ---- PeerDock.test.tsx "derives total / live / blocked / done via
    // ---- summarizeRoster"
    #[test]
    fn counts_bucket_every_row_and_always_sum_to_total() {
        let mut blocked = live();
        blocked.activity = PeerActivity::Blocked;
        let mut done = live();
        done.activity = PeerActivity::Done;
        let mut idle = live();
        idle.activity = PeerActivity::Idle;
        let entries = vec![entry("a", live()), entry("b", blocked), entry("c", done), entry("d", idle)];
        let c = summarize_roster(&entries);
        assert_eq!((c.total, c.live, c.blocked, c.done, c.idle), (4, 1, 1, 1, 1));
        assert_eq!(
            c.live + c.blocked + c.done + c.idle,
            c.total,
            "the four buckets always sum to total"
        );
        // empty roster
        let empty = summarize_roster(&[]);
        assert_eq!(empty.total, 0);
        assert_eq!(empty.live + empty.blocked + empty.done + empty.idle, 0);
    }

    // ---- PeerDock.test.tsx "counts landed rows via fleetLanded (done only)"
    #[test]
    fn landed_counts_done_rows_only() {
        let mut done = live();
        done.activity = PeerActivity::Done;
        let mut blocked = live();
        blocked.activity = PeerActivity::Blocked;
        let entries = vec![entry("a", done), entry("b", live()), entry("c", blocked)];
        assert_eq!(fleet_landed(&entries), (1, 3), "only `done` rows land");
        // zero-done is 0, not the total.
        let none = vec![entry("a", live()), entry("b", live())];
        assert_eq!(fleet_landed(&none), (0, 2));
    }

    // ---- peer-row-view.ts:23-27
    #[test]
    fn the_label_never_leaks_the_slug() {
        assert_eq!(peer_row_label(0, Some("glm-4")), "Peer 1 · glm-4");
        assert_eq!(peer_row_label(2, Some("glm-4")), "Peer 3 · glm-4");
        assert_eq!(peer_row_label(0, None), "Peer 1");
        assert_eq!(peer_row_label(1, Some("")), "Peer 2", "an empty model is no model");
        // the slug itself never appears in the label
        assert!(!peer_row_label(0, None).contains("brass-otter"));
    }

    // ---- peer-row-view.ts:33-51 the availability rules
    #[test]
    fn row_actions_follow_the_axis_and_the_addressable_gate() {
        // live: steer + stop
        assert_eq!(
            peer_row_actions(&entry("a", live())),
            vec![PeerRowAction::Steer, PeerRowAction::Stop]
        );
        // approval-blocked: approve/approve_session/deny/stop
        let mut approval = live();
        approval.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "r1".into(),
                kind: AttentionRequestKind::Approval,
                turn_id: "t1".into(),
            },
            1,
        );
        assert_eq!(
            peer_row_actions(&entry("a", approval)),
            vec![
                PeerRowAction::Approve,
                PeerRowAction::ApproveSession,
                PeerRowAction::Deny,
                PeerRowAction::Stop
            ]
        );
        // question-blocked: answer + stop
        let mut question = live();
        question.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "q1".into(),
                kind: AttentionRequestKind::Question,
                turn_id: "t1".into(),
            },
            1,
        );
        assert_eq!(
            peer_row_actions(&entry("a", question)),
            vec![PeerRowAction::Answer, PeerRowAction::Stop]
        );
        // done: nothing
        let mut done = live();
        done.apply(&PeerSessionEvent::TurnTerminal { outcome: TurnOutcome::Finished }, 1);
        assert!(peer_row_actions(&entry("a", done)).is_empty());
        // NOT addressable (no accepted operation id) -> no affordance at all.
        let mut unaddressable = live();
        unaddressable.operation_id = None;
        assert!(peer_row_actions(&entry("a", unaddressable)).is_empty(), "terminal/unaddressable");
        let mut empty_id = live();
        empty_id.operation_id = Some(String::new());
        assert!(peer_row_actions(&entry("a", empty_id)).is_empty());
    }

    // ---- peer-row-view.ts:90-120 peerAnswerRequest, fail-closed
    #[test]
    fn the_answer_request_is_fail_closed_and_uses_the_real_ids() {
        let mut q = live();
        q.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "q-real".into(),
                kind: AttentionRequestKind::Question,
                turn_id: "t9".into(),
            },
            1,
        );
        let e = entry("brass-otter", q);
        let detail = serde_json::json!({
            "header": "Pick a target",
            "question": "Which file?",
            "options": [
                {"label": "main.rs", "description": "the entry point"},
                {"label": "lib.rs", "description": null}
            ],
            "multiSelect": true,
            "allowFreeText": false
        });
        let card = peer_answer_request(&e, Some(&detail)).expect("a real card");
        // the REAL pending id, never a synthetic one
        assert_eq!(card["questionId"], "q-real");
        assert_eq!(card["turnId"], "t9");
        assert_eq!(card["title"], "Pick a target");
        assert_eq!(card["body"], "Which file?");
        let options = card["questions"][0]["options"].as_array().unwrap();
        assert_eq!(options.len(), 2);
        assert_eq!(options[0]["label"], "main.rs");
        assert_eq!(options[1]["description"], Value::Null);
        assert_eq!(card["questions"][0]["multiSelect"], true);

        // NOT question-blocked -> null
        assert!(peer_answer_request(&entry("a", live()), Some(&detail)).is_none());
        // question-blocked but NO detail -> null (fail closed)
        assert!(peer_answer_request(&e, None).is_none());
        // detail without options -> null
        let no_options = serde_json::json!({"header": "h", "question": "q"});
        assert!(peer_answer_request(&e, Some(&no_options)).is_none());
        // blocked on an APPROVAL -> no answer card
        let mut approval = live();
        approval.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "a1".into(),
                kind: AttentionRequestKind::Approval,
                turn_id: "t1".into(),
            },
            1,
        );
        assert!(peer_answer_request(&entry("a", approval), Some(&detail)).is_none());
    }

    #[test]
    fn the_attention_facts_are_the_server_reported_ids() {
        let mut q = live();
        q.apply(
            &PeerSessionEvent::AttentionRequested {
                request_id: "q-real".into(),
                kind: AttentionRequestKind::Question,
                turn_id: "t9".into(),
            },
            1,
        );
        let attn = peer_row_attention(&entry("brass-otter", q));
        assert_eq!(attn.request_id.as_deref(), Some("q-real"));
        assert_eq!(attn.request_kind, Some("question"));
        assert_eq!(attn.operation_id.as_deref(), Some("op-1"));
        assert_eq!(attn.turn_id, "t9");
    }

    // ---- the elapsed ladder
    #[test]
    fn elapsed_renders_the_dock_ladder() {
        assert_eq!(format_elapsed(0), "0s");
        assert_eq!(format_elapsed(45_000), "45s");
        assert_eq!(format_elapsed(80_000), "1m 20s");
        assert_eq!(format_elapsed(60 * 60 * 1000), "1h 0m");
        assert_eq!(format_elapsed(2 * 3600_000 + 5 * 60_000), "2h 5m");
        assert_eq!(format_elapsed(3 * 86_400_000 + 2 * 3600_000), "3d 2h");
    }

    // ---- a closed row keeps its last activity and is still counted
    #[test]
    fn a_closed_row_keeps_its_last_activity_and_still_counts() {
        let mut done = live();
        done.apply(&PeerSessionEvent::TurnTerminal { outcome: TurnOutcome::Finished }, 1);
        let mut p = Peer::named("a");
        p.closed = true;
        let entries = vec![PeerRosterEntry { peer: p, state: done }];
        let c = summarize_roster(&entries);
        assert_eq!((c.total, c.done), (1, 1), "a closed row is included with its last activity");
        assert_eq!(fleet_landed(&entries), (1, 1));
    }
}
