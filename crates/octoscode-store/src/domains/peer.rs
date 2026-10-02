//! `peer` state: staged/closed peer sessions.
//!
//! Fed by the client domain's `peer/staged` + `peer/closed` handlers
//! (`crates/octoscode-client/src/domains/peer.rs`). Shaped for the peer
//! roster/dock the parity matrix describes: the slug (the peer's primary
//! address), its session topic, the ORIGINATING session, the durable brief
//! path, the fence branch, and whether the model has since closed it.
//!
//! Every field's source is `PeerStagedEvent`
//! (`octos-core crates/octos-core/src/ui_protocol.rs:6482` @ pin `a6ea8505`);
//! `peer/closed` (`:6514`) only flips [`Peer::closed`].
use std::collections::HashMap;
use std::sync::Mutex;

/// One peer, as `peer/staged` / `peer/closed` describe it.
///
/// `name` / `closed` lead deliberately: the roster row reads the address
/// (slug) and the closed flag first, and the open count is `!closed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// The directory slug reserved under the profile's `peers/` root — the
    /// peer's address (`PeerStagedEvent.slug`).
    pub name: String,
    /// Set by `peer/closed`; an open peer is not yet closed.
    pub closed: bool,
    /// The peer session's topic (`peer-<slug>`) — a payload field, NOT the
    /// routing key (routing stays on the originating `session_id`).
    pub topic: Option<String>,
    /// Profile the peer session runs under.
    pub profile_id: Option<String>,
    /// The ORIGINATING session — the conversation whose turn staged it.
    pub origin_session_id: Option<String>,
    /// Absolute path of the durable brief (`peers/<slug>/brief.md`).
    pub brief_path: Option<String>,
    /// Working directory of the peer session (worktree checkout when fenced).
    pub cwd: Option<String>,
    /// Fence branch (`peer/<slug>`) when a worktree was created.
    pub worktree_branch: Option<String>,
    /// When the roster first saw the peer (ms epoch) — the fleet row's
    /// elapsed segment (`formatElapsed`, peer-row-view.ts:127; #32c item 11).
    pub staged_at_ms: u64,
}

/// Wall-clock now in ms (falls back to 0 if the clock is before the epoch).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Peer {
    /// A peer known only by its address — the legacy `stage` shape.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            closed: false,
            topic: None,
            profile_id: None,
            origin_session_id: None,
            brief_path: None,
            cwd: None,
            worktree_branch: None,
            staged_at_ms: now_ms(),
        }
    }
}

/// The peer domain.
#[derive(Debug, Default)]
pub struct Peers {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    peers: HashMap<String, Peer>,
    /// A10 — the peer manager's roster (`PeerManager.#peers`), in first-seen
    /// order, keyed by identity.
    rows: Vec<PeerRow>,
    /// The activity a row had right before a `blocked` wait
    /// (`PeerManager.#preBlock`), restored when the wait resolves.
    pre_block: HashMap<String, Activity>,
    /// Closed identities (`PeerManager.#closed`): a replayed `peer/staged`
    /// never reopens them.
    tombstones: HashMap<String, u64>,
    close_revision: u64,
    /// The walked `session/driver/get` inventory (the Fleet's acceptance facts).
    inventory: Option<FleetInventory>,
    /// Bumped on every roster / inventory change (cheap UI change detection).
    revision: u64,
}

impl Peers {
    /// Record a `peer/staged` fact, keyed by the slug.
    ///
    /// A replayed `peer/staged` (the event is durable, so reconnect replay
    /// redelivers it) must NOT reopen a peer the model has already closed:
    /// the existing `closed` flag is preserved across the update.
    pub fn observe_staged(&self, peer: Peer) {
        let mut i = self.inner.lock().unwrap();
        let closed = i.peers.get(&peer.name).map(|p| p.closed).unwrap_or(false);
        i.peers.insert(peer.name.clone(), Peer { closed, ..peer });
    }

    /// Record `peer/closed` for `name`; an unknown slug is recorded closed so
    /// the roster can still drop it.
    pub fn mark_closed(&self, name: &str) {
        let mut i = self.inner.lock().unwrap();
        match i.peers.get_mut(name) {
            Some(p) => p.closed = true,
            None => {
                i.peers
                    .insert(name.to_owned(), Peer { closed: true, ..Peer::named(name) });
            }
        }
    }

    /// One peer by its address.
    pub fn get(&self, name: &str) -> Option<Peer> {
        self.inner.lock().unwrap().peers.get(name).cloned()
    }

    /// Insert or replace the whole row (used by tests and direct callers).
    pub fn upsert(&self, peer: Peer) {
        self.inner.lock().unwrap().peers.insert(peer.name.clone(), peer);
    }

    /// Stage a peer by address alone (the first card's shape; kept).
    pub fn stage(&self, name: String) {
        self.observe_staged(Peer::named(name));
    }

    /// Close a peer by address (the first card's shape; kept).
    pub fn close(&self, name: &str) {
        self.mark_closed(name);
    }

    pub fn list(&self) -> Vec<Peer> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<Peer> = i.peers.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn open_count(&self) -> usize {
        self.inner.lock().unwrap().peers.values().filter(|p| !p.closed).count()
    }
}

// ===========================================================================
// A10 — the peer manager's roster (web `features/peers/peer-manager.ts` +
// `peer-roster.ts`): one row per staged / dispatched peer Session, its
// lifecycle `status`, the orthogonal live-run `activity` folded from the
// peer Session's OWN frames, the pending operator request, the accepted
// dispatch identity, tokens, outcome and acknowledgment. Pure data + the fold
// rules; the transport work (open, kickoff, dispatch, control) is the
// module's.
// ===========================================================================

/// Where a roster row came from (`PeerRosterEntry.origin`, + the dispatch
/// adoption the external driver produces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Prepare,
    Staged,
    Dispatch,
}

/// The lifecycle axis (`PeerRosterEntry.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    Opening,
    Started,
    Failed,
    Unknown,
    Closed,
}

/// The live-run axis (`PeerActivity`, peer-roster.ts:40).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Live,
    Blocked,
    Done,
}

/// What a `blocked` row waits on (`PeerAttentionRequestKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Approval,
    Question,
}

/// The terminal outcome of the row's last turn (`PeerTurnOutcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Finished,
    Stopped,
    Failed,
}

/// A control ACKNOWLEDGMENT — never an outcome (`PeerControlAcknowledgment`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ack {
    /// "Sent" for steer / answer / approval.
    Sent,
    /// "Stop requested" for an interrupt.
    StopRequested,
}

/// The contents of a pending approval (`PeerApprovalDetail`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApprovalDetail {
    pub tool_name: String,
    pub target: Option<String>,
    pub scope: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
}

/// The contents of a pending question (`PeerQuestionDetail`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestionDetail {
    pub header: Option<String>,
    pub question: Option<String>,
    /// `(label, description)`.
    pub options: Vec<(String, Option<String>)>,
    pub multi_select: bool,
    pub allow_free_text: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestDetail {
    Approval(ApprovalDetail),
    Question(QuestionDetail),
}

/// The console row's LAST control outcome (`PeerRowControlState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowControl {
    Sending,
    Receipt { duplicate: bool },
    Refused { kind: String },
}

/// One roster row (`PeerRosterEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRow {
    /// The row key: the staged / adopted session id
    /// (`<master base>#peer-<slug>`, `peerIdentityForTopic`).
    pub identity: String,
    pub profile_id: String,
    pub topic: String,
    pub slug: String,
    pub cwd: String,
    pub brief_path: String,
    /// The brief this row was staged with (the kickoff's source).
    pub brief: String,
    pub origin: Origin,
    /// The turn the row last targeted (the adopted / kickoff turn).
    pub turn_id: String,
    pub status: RowStatus,
    pub activity: Activity,
    /// Wall-clock ms the row entered `Opening` (the "Still starting…" clock).
    pub opening_since_ms: u64,
    /// Wall-clock ms the row last became `Started`.
    pub opened_at_ms: Option<u64>,
    /// Wall-clock ms of the latest turn terminal (frozen for `done`).
    pub finished_at_ms: Option<u64>,
    pub output_tokens: u64,
    pub request_id: Option<String>,
    pub request_kind: Option<RequestKind>,
    pub request_detail: Option<RequestDetail>,
    /// The accepted dispatch's RESOLVED model.
    pub model: Option<String>,
    /// The accepted dispatch's operation id (the control target).
    pub operation_id: Option<String>,
    /// The dispatch's goal id, when one was carried.
    pub goal_id: Option<String>,
    pub accepted_at_ms: Option<u64>,
    pub acknowledgment: Option<Ack>,
    pub outcome: Option<Outcome>,
    /// A replacement turn started after an acknowledged action (§4.3).
    pub turn_changed_since_ack: bool,
    pub error: Option<String>,
    pub can_retry: bool,
    pub control: Option<RowControl>,
}

impl PeerRow {
    /// A fresh `opening` row (`PeerManager.#stage`).
    pub fn opening(identity: &str, slug: &str, origin: Origin, turn_id: &str, now_ms: u64) -> Self {
        Self {
            identity: identity.to_owned(),
            profile_id: String::new(),
            topic: format!("peer-{slug}"),
            slug: slug.to_owned(),
            cwd: String::new(),
            brief_path: String::new(),
            brief: String::new(),
            origin,
            turn_id: turn_id.to_owned(),
            status: RowStatus::Opening,
            activity: Activity::Idle,
            opening_since_ms: now_ms,
            opened_at_ms: None,
            finished_at_ms: None,
            output_tokens: 0,
            request_id: None,
            request_kind: None,
            request_detail: None,
            model: None,
            operation_id: None,
            goal_id: None,
            accepted_at_ms: None,
            acknowledgment: None,
            outcome: None,
            turn_changed_since_ack: false,
            error: None,
            can_retry: false,
            control: None,
        }
    }
}

/// One event from a peer's OWN Session (`PeerSessionEvent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerSessionEvent {
    TurnStarted { turn_id: Option<String> },
    AttentionRequested { request_id: Option<String>, kind: Option<RequestKind>, detail: Option<RequestDetail> },
    AttentionResolved,
    TurnTerminal { outcome: Outcome, error: Option<String> },
    ControlAck { interrupt: bool },
    Usage { output_tokens: u64 },
}

/// One walked inventory operation (`DriverInventoryOperation`): every
/// acceptance field preserved verbatim; identity is never rebuilt from a slug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryOp {
    pub operation_id: String,
    pub slug: String,
    pub lifecycle: String,
    pub adopted_session_id: String,
    pub adopted_turn_id: String,
    pub workspace_root: String,
    pub model: String,
    pub model_lane: String,
    pub goal_id: Option<String>,
    pub accepted_at_ms: u64,
}

/// The public driver disclosure (`DriverInventoryDisclosure`): presentation
/// only — never a proof or token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    /// `internal` | `external`.
    pub mode: String,
    /// `none` | `interrupted` | `recovery_required`.
    pub recovery: String,
    /// `(driver_id, epoch, revision, lease_expires_at_ms)`.
    pub binding: Option<(String, u64, u64, u64)>,
}

/// The walked `session/driver/get` inventory (`DriverInventoryState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetInventory {
    Loading,
    Complete {
        session_id: String,
        snapshot: String,
        observed_revision: String,
        operations: Vec<InventoryOp>,
        disclosure: Disclosure,
        completed_at_ms: u64,
    },
    /// A failed walk (a bounded constant reason, never server copy).
    Error { session_id: String, reason: String },
}

impl Peers {
    /// The roster in first-seen order (`PeerManagerSnapshot.peers`).
    pub fn rows(&self) -> Vec<PeerRow> {
        self.inner.lock().unwrap().rows.clone()
    }

    /// One roster row by identity.
    pub fn row(&self, identity: &str) -> Option<PeerRow> {
        self.inner.lock().unwrap().rows.iter().find(|r| r.identity == identity).cloned()
    }

    /// One roster row by its (adopted) slug.
    pub fn row_by_slug(&self, slug: &str) -> Option<PeerRow> {
        self.inner.lock().unwrap().rows.iter().find(|r| r.slug == slug).cloned()
    }

    /// The roster/inventory revision (bumped on every change).
    pub fn roster_revision(&self) -> u64 {
        self.inner.lock().unwrap().revision
    }

    /// Whether `identity` was closed (a tombstone survives acknowledgment).
    pub fn is_tombstoned(&self, identity: &str) -> bool {
        self.inner.lock().unwrap().tombstones.contains_key(identity)
    }

    /// Stage ONE row (`PeerManager.#stage`). Returns `false` when the identity
    /// is tombstoned (a replayed `peer/staged` never reopens a closed peer) or
    /// already present (dedupe: one row, one kickoff UUID per identity). A
    /// NEW user prepare (`authorize_reuse`) may reuse a closed identity.
    pub fn stage_row(&self, row: PeerRow, authorize_reuse: bool) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.tombstones.contains_key(&row.identity) {
            if !authorize_reuse {
                return false;
            }
            i.tombstones.remove(&row.identity);
            i.rows.retain(|r| r.identity != row.identity);
        }
        if i.rows.iter().any(|r| r.identity == row.identity) {
            return false;
        }
        i.rows.push(row);
        i.revision += 1;
        true
    }

    /// Apply `f` to the row named `identity` (no-op when absent).
    pub fn update_row(&self, identity: &str, f: impl FnOnce(&mut PeerRow)) -> bool {
        let mut i = self.inner.lock().unwrap();
        let Some(row) = i.rows.iter_mut().find(|r| r.identity == identity) else { return false };
        f(row);
        i.revision += 1;
        true
    }

    /// Re-key a row to the server-ADOPTED identity (finding 2920 (b): the
    /// receipt's own slug/session is authoritative) and stamp the confirmed
    /// start (`PeerManager.#open` on `started`).
    #[allow(clippy::too_many_arguments)]
    pub fn mark_started(
        &self,
        identity: &str,
        adopted_identity: &str,
        adopted_slug: &str,
        operation_id: Option<&str>,
        adopted_turn_id: Option<&str>,
        model: Option<&str>,
        now_ms: u64,
    ) -> bool {
        let mut i = self.inner.lock().unwrap();
        let Some(pos) = i.rows.iter().position(|r| r.identity == identity) else { return false };
        // A row already keyed by the adopted identity (an inventory/replay
        // adoption) is merged, never duplicated.
        if identity != adopted_identity {
            if let Some(dup) = i.rows.iter().position(|r| r.identity == adopted_identity) {
                if dup != pos {
                    i.rows.remove(dup);
                }
            }
        }
        let Some(row) = i.rows.iter_mut().find(|r| r.identity == identity) else { return false };
        row.identity = adopted_identity.to_owned();
        row.slug = adopted_slug.to_owned();
        row.topic = format!("peer-{adopted_slug}");
        row.status = RowStatus::Started;
        row.error = None;
        row.can_retry = false;
        row.opened_at_ms = Some(now_ms);
        if let Some(op) = operation_id {
            row.operation_id = Some(op.to_owned());
        }
        if let Some(t) = adopted_turn_id.filter(|t| !t.is_empty()) {
            row.turn_id = t.to_owned();
        }
        if let Some(m) = model.filter(|m| !m.is_empty()) {
            row.model = Some(m.to_owned());
        }
        i.revision += 1;
        true
    }

    /// A non-started settle (`not-started` → failed + retry; `unknown`).
    pub fn mark_not_started(&self, identity: &str, unknown: bool, error: &str) -> bool {
        self.update_row(identity, |r| {
            r.status = if unknown { RowStatus::Unknown } else { RowStatus::Failed };
            r.can_retry = !unknown;
            r.error = Some(error.to_owned());
        })
    }

    /// Close a row (`PeerManager.#close`): tombstoned, no operator request can
    /// still be outstanding, the operation id is dropped.
    pub fn close_row(&self, identity: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.tombstones.contains_key(identity) {
            return false;
        }
        i.close_revision += 1;
        let rev = i.close_revision;
        i.tombstones.insert(identity.to_owned(), rev);
        i.pre_block.remove(identity);
        if let Some(row) = i.rows.iter_mut().find(|r| r.identity == identity) {
            row.status = RowStatus::Closed;
            row.can_retry = false;
            row.error = None;
            row.operation_id = None;
            row.request_id = None;
            row.request_kind = None;
            row.request_detail = None;
        }
        i.revision += 1;
        true
    }

    /// Fold ONE event from a peer's own Session (`observeSessionEvent`,
    /// peer-manager.ts:323-457). Returns whether a live row owned it.
    pub fn observe_session_event(&self, identity: &str, event: &PeerSessionEvent, now_ms: u64) -> bool {
        let mut i = self.inner.lock().unwrap();
        let Some(pos) = i.rows.iter().position(|r| r.identity == identity && r.status != RowStatus::Closed) else {
            return false;
        };
        match event {
            PeerSessionEvent::TurnStarted { turn_id } => {
                i.pre_block.remove(identity);
                let row = &mut i.rows[pos];
                // A REPLACEMENT turn invalidates a pending acknowledgment.
                if let Some(t) = turn_id.as_deref().filter(|t| !t.is_empty() && *t != row.turn_id) {
                    row.turn_changed_since_ack = row.acknowledgment.is_some();
                    row.acknowledgment = None;
                    row.turn_id = t.to_owned();
                }
                // A fresh turn runs: the previous outcome no longer stands.
                row.outcome = None;
                row.activity = Activity::Live;
            }
            PeerSessionEvent::ControlAck { interrupt } => {
                let row = &mut i.rows[pos];
                row.acknowledgment = Some(if *interrupt { Ack::StopRequested } else { Ack::Sent });
                row.turn_changed_since_ack = false;
            }
            PeerSessionEvent::AttentionRequested { request_id, kind, detail } => {
                let prev = i.rows[pos].activity;
                if prev != Activity::Blocked {
                    i.pre_block.insert(identity.to_owned(), prev);
                }
                let row = &mut i.rows[pos];
                row.request_id = request_id.clone();
                row.request_kind = *kind;
                row.request_detail = detail.clone();
                // The wait supersedes a pending acknowledgment.
                row.acknowledgment = None;
                row.activity = Activity::Blocked;
            }
            PeerSessionEvent::AttentionResolved => {
                let restored = i.pre_block.remove(identity);
                let row = &mut i.rows[pos];
                if restored.is_some() || row.activity == Activity::Blocked {
                    row.request_id = None;
                    row.request_kind = None;
                    row.request_detail = None;
                    row.activity = restored.unwrap_or(Activity::Idle);
                }
            }
            PeerSessionEvent::Usage { output_tokens } => {
                if *output_tokens > 0 {
                    let row = &mut i.rows[pos];
                    row.output_tokens = row.output_tokens.saturating_add(*output_tokens);
                }
            }
            PeerSessionEvent::TurnTerminal { outcome, error } => {
                i.pre_block.remove(identity);
                let row = &mut i.rows[pos];
                row.request_id = None;
                row.request_kind = None;
                row.request_detail = None;
                row.acknowledgment = None;
                row.turn_changed_since_ack = false;
                row.outcome = Some(*outcome);
                if *outcome == Outcome::Failed {
                    if let Some(e) = error {
                        row.error = Some(e.clone());
                    }
                }
                row.activity = Activity::Done;
                row.finished_at_ms = Some(now_ms);
            }
        }
        i.revision += 1;
        true
    }

    /// `clearFinished` (TUI `/peer clear`): prune `done` / `closed` rows only;
    /// tombstones survive. Returns how many were pruned.
    pub fn clear_finished(&self) -> usize {
        let mut i = self.inner.lock().unwrap();
        let before = i.rows.len();
        let gone: Vec<String> = i
            .rows
            .iter()
            .filter(|r| r.activity == Activity::Done || r.status == RowStatus::Closed)
            .map(|r| r.identity.clone())
            .collect();
        i.rows.retain(|r| !gone.contains(&r.identity));
        for g in &gone {
            i.pre_block.remove(g);
        }
        let removed = before - i.rows.len();
        if removed > 0 {
            i.revision += 1;
        }
        removed
    }

    /// Drop ONE row from the roster (an acknowledged close / a capture
    /// trim); its tombstone, if any, survives.
    pub fn clear_roster_row(&self, identity: &str) {
        let mut i = self.inner.lock().unwrap();
        i.rows.retain(|r| r.identity != identity);
        i.pre_block.remove(identity);
        i.revision += 1;
    }

    /// Record the walked inventory (or its loading / error state).
    pub fn set_inventory(&self, inventory: Option<FleetInventory>) {
        let mut i = self.inner.lock().unwrap();
        i.inventory = inventory;
        i.revision += 1;
    }

    pub fn inventory(&self) -> Option<FleetInventory> {
        self.inner.lock().unwrap().inventory.clone()
    }

    /// Full semantic retirement (`PeerManager.clear`): a new owner never sees
    /// the previous owner's rows.
    pub fn clear_roster(&self) {
        let mut i = self.inner.lock().unwrap();
        i.rows.clear();
        i.pre_block.clear();
        i.tombstones.clear();
        i.inventory = None;
        i.revision += 1;
    }
}

#[cfg(test)]
mod roster_tests {
    use super::*;

    fn staged(id: &str) -> PeerRow {
        PeerRow::opening(id, id.rsplit("peer-").next().unwrap_or(id), Origin::Staged, "t0", 1)
    }

    #[test]
    fn staging_dedupes_replay_and_tombstones_block_reuse() {
        let p = Peers::default();
        assert!(p.stage_row(staged("m#peer-a"), false));
        assert!(!p.stage_row(staged("m#peer-a"), false), "a replayed staged event is one row");
        assert!(p.close_row("m#peer-a"));
        assert!(!p.stage_row(staged("m#peer-a"), false), "a closed peer stays closed on replay");
        assert!(p.stage_row(staged("m#peer-a"), true), "a NEW prepare receipt may reuse it");
        assert_eq!(p.rows().len(), 1);
    }

    #[test]
    fn the_activity_axis_follows_the_peer_sessions_own_frames() {
        let p = Peers::default();
        p.stage_row(staged("m#peer-a"), false);
        assert!(p.mark_started("m#peer-a", "m#peer-a", "a", Some("op-1"), Some("turn-1"), Some("gpt"), 5));
        let ev = |e: PeerSessionEvent| assert!(p.observe_session_event("m#peer-a", &e, 9));
        ev(PeerSessionEvent::TurnStarted { turn_id: Some("turn-1".into()) });
        assert_eq!(p.row("m#peer-a").unwrap().activity, Activity::Live);
        ev(PeerSessionEvent::ControlAck { interrupt: false });
        assert_eq!(p.row("m#peer-a").unwrap().acknowledgment, Some(Ack::Sent));
        ev(PeerSessionEvent::AttentionRequested {
            request_id: Some("ap-1".into()),
            kind: Some(RequestKind::Approval),
            detail: None,
        });
        let r = p.row("m#peer-a").unwrap();
        assert_eq!((r.activity, r.request_id.as_deref(), r.acknowledgment), (Activity::Blocked, Some("ap-1"), None));
        ev(PeerSessionEvent::AttentionResolved);
        assert_eq!(p.row("m#peer-a").unwrap().activity, Activity::Live, "restores the pre-block activity");
        ev(PeerSessionEvent::Usage { output_tokens: 1200 });
        ev(PeerSessionEvent::TurnTerminal { outcome: Outcome::Stopped, error: None });
        let r = p.row("m#peer-a").unwrap();
        assert_eq!(
            (r.activity, r.outcome, r.output_tokens, r.finished_at_ms),
            (Activity::Done, Some(Outcome::Stopped), 1200, Some(9))
        );
        // A replacement turn clears the outcome and flags a stale ack.
        ev(PeerSessionEvent::ControlAck { interrupt: true });
        ev(PeerSessionEvent::TurnStarted { turn_id: Some("turn-2".into()) });
        let r = p.row("m#peer-a").unwrap();
        assert!(r.turn_changed_since_ack && r.acknowledgment.is_none() && r.outcome.is_none());
        assert_eq!(r.turn_id, "turn-2");
        // A closed row owns no further events.
        p.close_row("m#peer-a");
        assert!(!p.observe_session_event("m#peer-a", &PeerSessionEvent::AttentionResolved, 10));
    }

    #[test]
    fn the_adopted_identity_rekeys_the_row() {
        let p = Peers::default();
        p.stage_row(PeerRow::opening("m#peer-staged", "staged", Origin::Prepare, "k", 1), false);
        assert!(p.mark_started("m#peer-staged", "m#peer-op-7", "op-7", Some("op"), Some("t"), None, 2));
        assert!(p.row("m#peer-staged").is_none());
        assert_eq!(p.row_by_slug("op-7").unwrap().status, RowStatus::Started);
    }
}
