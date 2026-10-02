//! Board-3 screen 4 — the FLEET destination (A4's pane; A10 completes it to
//! the web's `features/fleet/FleetView.tsx` + `fleet-model.ts` +
//! `fleet-facts.ts` + `FleetPane.tsx`, and `features/control/*`).
//!
//! Reached from the sidebar footer's "Fleet" entry; the pane REPLACES the
//! chat area and its way out is "Back" (`App.tsx:3048-3136`).
//!
//! * **rows** — the UNION of the walked `session/driver/get` acceptance facts
//!   with the peer manager's roster, keyed by the EXACT adopted session id
//!   (`unionFleetFacts`, `fleet-facts.ts:239-312`), labelled `Peer N · model`
//!   (never a slug), status words from the peer sessions' OWN events
//!   (Requested / Starting / "Still starting…" past 15 s / Working / Waiting
//!   for your approval|answer / Finished / Stopped / Failed / Outcome unknown);
//! * **grouping** — one group per goal id (insertion order, the goal-less
//!   "Peers" group last), the §3 rank order stable within a rank, terminal
//!   rows under each group's own "Finished (n)" (closed by default)
//!   (`fleetGroupPeers`, `fleet-model.ts:219-256`);
//! * **row actions** — Approve / Deny only while waiting for approval, Stop
//!   while running, Steer (the row's OWN text) only while working, each with
//!   the web's disabled reason; EXACTLY ONE `peer/control` per activation
//!   (`screens::fleet_driver::row_control`);
//! * **Start** — the lane picker sourced ONLY from the advertised
//!   `profile/sub_providers/list` keys (no default lane; a withdrawn lane
//!   collapses to none), Start = acquire (CAS) → prepare → ONE dispatch with
//!   the Start's minted operation id; a refusal keeps the brief and shows
//!   the bounded label; an uncertain outcome offers Retry (SAME id) and
//!   Dismiss (`fleet-actions.ts`, `FleetView.tsx:398-473`);
//! * **announcements** — the first row that newly waits for you or ends is
//!   announced once (`fleetAnnouncement`, keyed by the row, not its label);
//! * **Advanced** — the session controller (driver disclosure, the control
//!   seat, the peer controller console with its staging + roster), behind a
//!   disclosure that starts closed (`FleetView.tsx:556-592`,
//!   `SessionControlBar.tsx:786-952`) — see [`super::fleet_console`].
use std::collections::HashMap;

use octoscode_store::domains::peer::{Activity, Ack, FleetInventory, Outcome, PeerRow, RequestKind, RowControl, RowStatus};
use octoscode_store::Store;

use super::fleet_copy::{t, t1};
use super::host::Outcome as HostOutcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};
use crate::screens::fleet_driver::{self, StartOutcome, Staged};
use crate::screens::peers::{self, RowAction};

/// §4.3: "Still starting…" once Starting exceeds 15 s.
pub const SLOW_START_MS: u64 = 15_000;

// ------------------------------------------------------------ status words

/// The design's status vocabulary (`FleetStatusWord`, fleet-model.ts:19-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Requested,
    Starting,
    StillStarting,
    Working,
    WaitingApproval,
    WaitingAnswer,
    Finished,
    Stopped,
    Failed,
    Unknown,
}

impl Status {
    /// The English source word (the catalog key).
    pub fn word(self) -> &'static str {
        match self {
            Status::Requested => "Requested",
            Status::Starting => "Starting",
            Status::StillStarting => "Still starting…",
            Status::Working => "Working",
            Status::WaitingApproval => "Waiting for your approval",
            Status::WaitingAnswer => "Waiting for your answer",
            Status::Finished => "Finished",
            Status::Stopped => "Stopped",
            Status::Failed => "Failed",
            Status::Unknown => "Outcome unknown",
        }
    }
    /// §3 rank (`STATUS_RANK` with the derived aliases).
    pub fn rank(self) -> u8 {
        match self {
            Status::WaitingApproval => 0,
            Status::WaitingAnswer => 1,
            Status::Working => 2,
            Status::Starting | Status::StillStarting | Status::Requested => 3,
            Status::Finished => 4,
            Status::Stopped => 5,
            Status::Failed | Status::Unknown => 6,
        }
    }
    /// `FLEET_TERMINAL_STATUSES`.
    pub fn terminal(self) -> bool {
        matches!(self, Status::Finished | Status::Stopped | Status::Failed | Status::Unknown)
    }
    /// §8: a visible glyph per status (never colour-only).
    pub fn glyph(self) -> &'static str {
        match self {
            Status::Requested | Status::Starting | Status::StillStarting => "○",
            Status::Working => "✻",
            Status::WaitingApproval | Status::WaitingAnswer => "⚠",
            Status::Finished => "✓",
            Status::Stopped | Status::Failed => "✕",
            Status::Unknown => "?",
        }
    }
    /// The chip colours (fg, bg).
    fn tone(self) -> (&'static str, &'static str) {
        match self {
            Status::WaitingApproval | Status::WaitingAnswer => (tok::AMBER, tok::AMBER_BG),
            Status::Working => (tok::GREEN, tok::GREEN_BG),
            Status::Requested | Status::Starting | Status::StillStarting => (tok::BLUE, tok::BLUE_BG),
            Status::Finished => (tok::MUTED, tok::SURFACE2),
            Status::Stopped | Status::Failed | Status::Unknown => (tok::RED, tok::RED_BG),
        }
    }
}

/// One roster row's status word (`unionStatusWord` + `fleetStatusWord`):
/// blocked → waiting (answer for a question, else approval); else the row's
/// outcome when its turn ended; else its lifecycle — opening → Starting /
/// "Still starting…" past 15 s, started → Working, closed → Finished,
/// failed → Failed, unknown → Outcome unknown.
pub fn row_status(row: &PeerRow, now_ms: u64) -> Status {
    if row.activity == Activity::Blocked {
        return if row.request_kind == Some(RequestKind::Question) {
            Status::WaitingAnswer
        } else {
            Status::WaitingApproval
        };
    }
    if let Some(o) = row.outcome {
        return match o {
            Outcome::Finished => Status::Finished,
            Outcome::Stopped => Status::Stopped,
            Outcome::Failed => Status::Failed,
        };
    }
    match row.status {
        RowStatus::Opening => {
            if now_ms.saturating_sub(row.opening_since_ms) > SLOW_START_MS {
                Status::StillStarting
            } else {
                Status::Starting
            }
        }
        RowStatus::Started => Status::Working,
        RowStatus::Closed => Status::Finished,
        RowStatus::Failed => Status::Failed,
        RowStatus::Unknown => Status::Unknown,
    }
}

// ------------------------------------------------------------- the union

/// One Fleet row (`FleetRosterPeer` from the union).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRow {
    /// The union key: the EXACT adopted session id (roster identity fallback).
    pub key: String,
    /// The roster identity actions address (`None` for inventory-only rows).
    pub identity: Option<String>,
    pub slug: String,
    /// "Peer N · model" / "Peer N" — never the slug.
    pub label: String,
    /// The brief's first line (≤ 60 chars), or the event-derived fallback.
    pub title: String,
    pub status: Status,
    pub session_name: String,
    pub goal_id: Option<String>,
    pub elapsed_ms: u64,
    pub tokens: u64,
    /// An accepted operation id exists (the row is addressable).
    pub control_supported: bool,
    pub ack: Option<Ack>,
    pub turn_changed: bool,
    pub control: Option<RowControl>,
    pub error: Option<String>,
}

/// `fleetRowTitle` (fleet-model.ts:260-268): the brief's first line, 60
/// chars; blank → the event-derived fallback (`fleetRowFallbackTitle`).
pub fn row_title(brief: &str, label: &str, status: Status) -> String {
    let first = brief.split('\n').next().unwrap_or("").trim();
    if first.is_empty() {
        return format!("{label} — {}", t(status.word()));
    }
    first.chars().take(60).collect()
}

/// `unionFleetFacts` + `fleetRosterFromUnion`: the inventory facts first
/// (operation order), each matched to the roster row whose identity IS its
/// adopted session id; then every unclaimed roster row. Labels are assigned
/// after the union in that order.
pub fn rows(store: &Store, now_ms: u64) -> Vec<FleetRow> {
    let roster = store.domains.peer.rows();
    let ops = match store.domains.peer.inventory() {
        Some(FleetInventory::Complete { operations, .. }) => operations,
        _ => Vec::new(),
    };
    let workspace = store
        .active_session()
        .and_then(|s| store.domains.session.workspace_root(&s))
        .unwrap_or_default();
    let mut out: Vec<(FleetRow, Option<String>)> = Vec::new();
    let mut claimed: Vec<String> = Vec::new();
    for op in &ops {
        let r = roster.iter().find(|r| r.identity == op.adopted_session_id);
        if let Some(r) = r {
            claimed.push(r.identity.clone());
        }
        let status = r.map(|r| row_status(r, now_ms)).unwrap_or(Status::Requested);
        out.push((
            FleetRow {
                key: op.adopted_session_id.clone(),
                identity: r.map(|r| r.identity.clone()),
                slug: op.slug.clone(),
                label: String::new(),
                title: String::new(),
                status,
                session_name: ui::leaf(if op.workspace_root.is_empty() { &workspace } else { &op.workspace_root }),
                goal_id: op.goal_id.clone(),
                elapsed_ms: now_ms.saturating_sub(op.accepted_at_ms),
                tokens: r.map(|r| r.output_tokens).unwrap_or(0),
                control_supported: r.and_then(|r| r.operation_id.clone()).is_some() || !op.operation_id.is_empty(),
                ack: r.and_then(|r| r.acknowledgment),
                turn_changed: r.is_some_and(|r| r.turn_changed_since_ack),
                control: r.and_then(|r| r.control.clone()),
                error: r.and_then(|r| r.error.clone()),
            },
            Some(op.model.clone()).filter(|m| !m.is_empty()).or_else(|| r.and_then(|r| r.model.clone())),
        ));
        if let Some(r) = r {
            out.last_mut().unwrap().0.title = r.brief.clone();
        }
    }
    for r in roster.iter().filter(|r| !claimed.contains(&r.identity)) {
        let status = row_status(r, now_ms);
        let since = r.accepted_at_ms.or(r.opened_at_ms).unwrap_or(r.opening_since_ms);
        let end = if r.activity == Activity::Done { r.finished_at_ms.unwrap_or(now_ms) } else { now_ms };
        out.push((
            FleetRow {
                key: r.identity.clone(),
                identity: Some(r.identity.clone()),
                slug: r.slug.clone(),
                label: String::new(),
                title: r.brief.clone(),
                status,
                session_name: ui::leaf(if r.cwd.is_empty() { &workspace } else { &r.cwd }),
                goal_id: r.goal_id.clone(),
                elapsed_ms: end.saturating_sub(since),
                tokens: r.output_tokens,
                control_supported: r.operation_id.as_deref().is_some_and(|o| !o.is_empty()),
                ack: r.acknowledgment,
                turn_changed: r.turn_changed_since_ack,
                control: r.control.clone(),
                error: r.error.clone(),
            },
            r.model.clone(),
        ));
    }
    out.into_iter()
        .enumerate()
        .map(|(i, (mut row, model))| {
            row.label = peers::row_label(i, model.as_deref());
            let brief = std::mem::take(&mut row.title);
            row.title = row_title(&brief, &row.label, row.status);
            row
        })
        .collect()
}

/// One group (`FleetPeerGroup`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub goal_id: Option<String>,
    /// Indices into the row list: active rows in §3 rank order (stable).
    pub active: Vec<usize>,
    /// Terminal rows, in rank order.
    pub finished: Vec<usize>,
}

impl Group {
    pub fn key(&self) -> String {
        self.goal_id.clone().unwrap_or_else(|| "peers".into())
    }
}

/// `fleetGroupPeers` (fleet-model.ts:219-256).
pub fn group(rows: &[FleetRow]) -> Vec<Group> {
    let mut order: Vec<Option<String>> = Vec::new();
    for r in rows {
        if !order.contains(&r.goal_id) {
            order.push(r.goal_id.clone());
        }
    }
    if order.len() > 1 {
        if let Some(pos) = order.iter().position(Option::is_none) {
            let none = order.remove(pos);
            order.push(none);
        }
    }
    order
        .into_iter()
        .map(|goal| {
            let mut members: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].goal_id == goal).collect();
            members.sort_by_key(|&i| rows[i].status.rank());
            let (finished, active): (Vec<usize>, Vec<usize>) =
                members.into_iter().partition(|&i| rows[i].status.terminal());
            Group { goal_id: goal, active, finished }
        })
        .collect()
}

/// `fleetActionAvailability` (fleet-model.ts:364-382).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Availability {
    pub approve: bool,
    pub deny: bool,
    pub steer: bool,
    pub stop: bool,
}

pub fn availability(status: Status, steer_text: &str) -> Availability {
    let waiting_approval = status == Status::WaitingApproval;
    let waiting_answer = status == Status::WaitingAnswer;
    let working = status == Status::Working;
    let starting = matches!(status, Status::Starting | Status::StillStarting | Status::Requested);
    Availability {
        approve: waiting_approval,
        deny: waiting_approval,
        steer: working && !steer_text.trim().is_empty(),
        stop: starting || working || waiting_approval || waiting_answer,
    }
}

/// `fleetAnnouncement` (fleet-model.ts:442-459), keyed by the row (the web
/// keys by slug), so a renumbered label never fakes a change.
pub fn announce(prev: &[(String, Status)], rows: &[FleetRow]) -> Option<String> {
    for r in rows {
        let before = prev.iter().find(|(k, _)| k == &r.key).map(|(_, s)| *s);
        if before == Some(r.status) {
            continue;
        }
        let msg = match r.status {
            Status::WaitingApproval => t1("{value0} is waiting for your approval", &r.label),
            Status::WaitingAnswer => t1("{value0} is waiting for your answer", &r.label),
            Status::Finished => t1("{value0} finished", &r.label),
            Status::Stopped => t1("{value0} stopped", &r.label),
            Status::Failed => t1("{value0} failed", &r.label),
            _ => continue,
        };
        return Some(msg);
    }
    None
}

// ------------------------------------------------------------- the picker

/// One `profile/sub_providers/list` lane as the Start form summarises it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneInfo {
    pub key: String,
    pub provider: String,
    pub model: Option<String>,
    pub description: Option<String>,
}

impl LaneInfo {
    /// `provider/model` when both are reported, else the display name.
    pub fn title(&self) -> String {
        match (&self.model, self.provider.trim().is_empty()) {
            (Some(m), false) if !m.trim().is_empty() => format!("{}/{}", self.provider.trim(), m.trim()),
            _ => model_name(&self.key),
        }
    }
    /// The avatar's initial (the provider's, else the key's).
    pub fn initial(&self) -> String {
        let src = if self.provider.trim().is_empty() { &self.key } else { &self.provider };
        src.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into())
    }
}

/// `fleetModelName` (fleet-model.ts:318-328): `name-dd` → `name-d.d`, else
/// the key verbatim (no other digit pair is rewritten).
pub fn model_name(key: &str) -> String {
    if let Some((name, digits)) = key.rsplit_once('-') {
        let name_ok = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && name.chars().all(|c| c.is_ascii_alphanumeric())
            && !name.contains('-');
        if name_ok && digits.len() == 2 && digits.bytes().all(|b| b.is_ascii_digit()) {
            return format!("{name}-{}.{}", &digits[..1], &digits[1..]);
        }
    }
    key.to_owned()
}

/// `fleetModelOptions` (fleet-model.ts:338-355): the lane key as a suffix
/// only when two lanes share a display name.
pub fn model_options(keys: &[String]) -> Vec<(String, String)> {
    let names: Vec<String> = keys.iter().map(|k| model_name(k)).collect();
    keys.iter()
        .zip(&names)
        .map(|(k, n)| {
            let dup = names.iter().filter(|m| *m == n).count() > 1;
            (k.clone(), if dup { format!("{n} ({k})") } else { n.clone() })
        })
        .collect()
}

/// The lane read's phase for this session (`laneReadStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaneRead {
    /// Not read (no session / not advertised): the picker stays disabled.
    #[default]
    Unread,
    Loading,
    Ready,
    Empty,
}

// ------------------------------------------------------------- the Start

/// The Start form's machine (`FleetStartState`, fleet-actions.ts:98-145).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StartState {
    #[default]
    Idle,
    Requesting { lane: String, brief: String, operation_id: String },
    Failed { lane: String, brief: String, operation_id: String, kind: String },
    Unknown { lane: String, brief: String, operation_id: String },
}

impl StartState {
    pub fn requesting(&self) -> bool {
        matches!(self, StartState::Requesting { .. })
    }
}

// ------------------------------------------------------------- the state

/// All Fleet UI state (the values the protocol never carries).
#[derive(Debug, Clone, Default)]
pub struct FleetState {
    pub lanes: Vec<LaneInfo>,
    pub lane_read: LaneRead,
    /// The chosen lane KEY; "" = none (no implicit default).
    pub lane: String,
    pub picker_open: bool,
    pub brief: String,
    pub brief_snap: String,
    pub start: StartState,
    /// The prepared identity a same-id retry reuses.
    pub staged: Option<Staged>,
    pub start_dismissed: bool,
    /// Per-row steer drafts (live / snapshot), keyed by the row key.
    pub steer: HashMap<String, String>,
    pub steer_snap: HashMap<String, String>,
    /// Per-group Finished (n) disclosure (closed by default).
    pub finished_open: HashMap<String, bool>,
    pub announcement: Option<String>,
    pub seen: Vec<(String, Status)>,
    /// The last row-action copy per row key ("Sent", a refusal label…).
    pub row_note: HashMap<String, String>,
    /// The rows the last lowering drew (the `#row` index → key/identity).
    pub drawn: Vec<FleetRow>,
    pub advanced_open: bool,
    pub console: super::fleet_console::ConsoleState,
    pub content_x: f64,
    /// A gather (`peer/gather` -> one synthesis turn) is in flight.
    pub gathering: bool,
}

impl FleetState {
    /// The advertised keys (`peerLaneKeys`: verbatim, blanks dropped).
    pub fn lane_keys(&self) -> Vec<String> {
        self.lanes.iter().map(|l| l.key.clone()).filter(|k| !k.trim().is_empty()).collect()
    }
    /// `peerControllerStagedLane`: the chosen lane only while advertised.
    pub fn chosen_lane(&self) -> Option<String> {
        let keys = self.lane_keys();
        (!self.lane.is_empty() && keys.contains(&self.lane)).then(|| self.lane.clone())
    }
    /// Freeze the live input text into the DSL snapshots before a remount.
    pub fn snap_inputs(&mut self) {
        self.brief_snap = self.brief.clone();
        self.steer_snap = self.steer.clone();
        self.console.snap_inputs();
    }
}

/// `peerLaneSourceAdmitted`: the read needs the advertised method and a
/// confirmed profile.
pub fn lane_source_admitted(store: &Store, profile: &str) -> bool {
    !profile.trim().is_empty()
        && store
            .domains
            .config
            .supported_methods()
            .iter()
            .any(|m| m == octoscode_client::domains::external_driver::PROFILE_SUB_PROVIDERS_LIST)
}

/// `fleetStartAdmitted` (fleet-model.ts:296-306): control supported, an
/// advertised lane chosen, a non-blank brief.
pub fn start_admitted(st: &FleetState, store: &Store) -> bool {
    fleet_driver::control_ready(store) && st.chosen_lane().is_some() && !st.brief.trim().is_empty()
}

// ------------------------------------------------------------- transport

/// The lane read (`peer-lane-source.ts`): fail-closed when unadvertised.
pub async fn load_lanes(conv: &crate::flow::Conversation) -> Result<String, String> {
    use octoscode_client::domains::profile::{SubProvidersList, SubProvidersListParams};
    if !lane_source_admitted(&conv.store, &conv.profile()) {
        super::host::state().fleet.lane_read = LaneRead::Unread;
        return Ok("lane source not advertised".into());
    }
    super::host::state().fleet.lane_read = LaneRead::Loading;
    super::host::wake();
    let r = conv
        .client()
        .call::<SubProvidersList>(SubProvidersListParams { profile_id: Some(conv.profile()) })
        .await;
    let mut st = super::host::state();
    match r {
        Ok(v) => {
            st.fleet.lanes = v
                .sub_providers
                .into_iter()
                .filter(|l| !l.key.trim().is_empty())
                .map(|l| LaneInfo {
                    key: l.key,
                    provider: l.provider,
                    model: l.model,
                    description: l.description.filter(|d| !d.trim().is_empty()),
                })
                .collect();
            st.fleet.lane_read = if st.fleet.lanes.is_empty() { LaneRead::Empty } else { LaneRead::Ready };
            // A withdrawn pick collapses to none (never a substitute).
            if st.fleet.chosen_lane().is_none() {
                st.fleet.lane.clear();
            }
            st.fleet.snap_inputs();
            Ok(format!("{} lanes", st.fleet.lanes.len()))
        }
        Err(e) => {
            st.fleet.lane_read = LaneRead::Empty;
            Err(e.to_string())
        }
    }
}

/// Run ONE Start (or its same-id retry) and settle the machine.
pub async fn run_start(conv: &crate::flow::Conversation, operation_id: String, lane: String, brief: String) -> Result<String, String> {
    let (keys, mut staged) = {
        let st = super::host::state();
        (st.fleet.lane_keys(), st.fleet.staged.clone())
    };
    // The "Still starting…" boundary needs a repaint without any frame.
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(SLOW_START_MS + 300)).await;
        super::host::wake();
    });
    let outcome = fleet_driver::start(conv, &operation_id, &lane, &keys, &brief, &mut staged).await;
    let mut st = super::host::state();
    st.fleet.staged = staged;
    let msg = format!("{outcome:?}");
    match outcome {
        StartOutcome::Accepted { .. } => {
            // An accepted settle CLEARS the form; the next Start is new.
            st.fleet.start = StartState::Idle;
            st.fleet.staged = None;
            st.fleet.brief.clear();
            st.fleet.brief_snap.clear();
        }
        StartOutcome::Refused { kind } => {
            // A refusal KEEPS the brief (§6) and the bounded label.
            st.fleet.start = StartState::Failed { lane, brief, operation_id, kind };
            st.fleet.staged = None;
            st.fleet.snap_inputs();
        }
        StartOutcome::Unknown => {
            st.fleet.start = StartState::Unknown { lane, brief, operation_id };
            st.fleet.start_dismissed = false;
            st.fleet.snap_inputs();
        }
    }
    Ok(msg)
}

/// The gather's bounded outcomes (`App.tsx:1365-1395`).
pub const GATHER_QUEUED: &str = "Peer synthesis queued";
pub const GATHER_EMPTY: &str = "No peers staged on the blackboard.";
pub const GATHER_FAILED: &str =
    "Peer synthesis was not queued. Check this Session’s authority and write availability, then retry.";

/// `peer/gather` is advertised for this session (`canGather`).
pub fn gather_admitted(store: &Store) -> bool {
    store.active_session().is_some() && store.domains.config.supported_methods().iter().any(|m| m == "peer/gather")
}

/// The web's `/gather` (`gatherFromRecord`, `gather.ts:78-145`): ONE
/// `peer/gather` read of the profile blackboard, `composeGatherPrompt` (the
/// 64 KiB cap), then the synthesis as ONE ordinary turn — reading needs no
/// write capability; the synthesis is ordinary input.
pub async fn run_gather(conv: &crate::flow::Conversation) -> Result<String, String> {
    use octoscode_client::domains::peer::{PeerGather, PeerGatherParams};
    let settle = |copy: &str| {
        let mut st = super::host::state();
        st.fleet.gathering = false;
        st.fleet.announcement = Some(t(copy));
    };
    let Some(session) = conv.store.active_session() else {
        settle(GATHER_FAILED);
        return Err("no session".into());
    };
    let read = conv
        .client()
        .call::<PeerGather>(PeerGatherParams { session_id: session, profile_id: conv.profile(), slugs: None })
        .await;
    let peers = match read {
        Ok(r) => r.peers,
        Err(e) => {
            settle(GATHER_FAILED);
            return Err(e.to_string());
        }
    };
    if peers.is_empty() {
        settle(GATHER_EMPTY);
        return Ok("empty".into());
    }
    let text = match peers::compose_gather_prompt(&peers) {
        Ok(t) => t,
        Err(e) => {
            settle(GATHER_FAILED);
            return Err(e);
        }
    };
    match conv.start_turn(text).await {
        Ok(turn) => {
            settle(GATHER_QUEUED);
            Ok(format!("queued {turn} ({} peers)", peers.len()))
        }
        Err(e) => {
            settle(GATHER_FAILED);
            Err(e.to_string())
        }
    }
}

/// ONE row action through the production control chain.
pub async fn run_row(conv: &crate::flow::Conversation, key: String, identity: String, action: RowAction, text: String) -> Result<String, String> {
    let res = fleet_driver::row_control(conv, &identity, action, &text).await;
    let mut st = super::host::state();
    match &res {
        Ok(ack) => {
            st.fleet.row_note.insert(key.clone(), ack.clone());
            if action == RowAction::Steer {
                st.fleet.steer.remove(&key);
                st.fleet.steer_snap.remove(&key);
            }
        }
        Err(label) => {
            st.fleet.row_note.insert(key, label.clone());
        }
    }
    res
}

// ------------------------------------------------------------------ actions

/// Route one `b3.fleet.*` action (UI-local first; jobs for the wire).
pub fn perform(st: &mut FleetState, action: &str, index: usize, store: &Store) -> HostOutcome {
    use super::host::Job;
    // Every UI-local change remounts: freeze the typed text first.
    st.snap_inputs();
    if let Some(rest) = action.strip_prefix("b3.fleet.console.") {
        return super::fleet_console::perform(st, rest, index, store);
    }
    match action {
        "b3.fleet.lane.toggle" => {
            if st.lane_read == LaneRead::Ready {
                st.picker_open = !st.picker_open;
            }
            HostOutcome::Done
        }
        "b3.fleet.lane" => {
            if let Some(k) = st.lane_keys().get(index).cloned() {
                st.lane = k;
                st.picker_open = false;
                // An edit releases a settled Start (a NEW request).
                if !st.start.requesting() {
                    st.start = StartState::Idle;
                    st.staged = None;
                }
            }
            HostOutcome::Done
        }
        "b3.fleet.start" => {
            if st.start.requesting() || !start_admitted(st, store) {
                return HostOutcome::Done;
            }
            let Some(lane) = st.chosen_lane() else { return HostOutcome::Done };
            let operation_id = octoscode_client::domains::external_driver::new_operation_id();
            let brief = st.brief.clone();
            st.start = StartState::Requesting { lane: lane.clone(), brief: brief.clone(), operation_id: operation_id.clone() };
            st.staged = None;
            st.start_dismissed = false;
            HostOutcome::Spawn(Job::FleetStart { operation_id, lane, brief })
        }
        "b3.fleet.retry" => {
            // Retry from UNCERTAIN re-enters requesting with the SAME id.
            let StartState::Unknown { lane, brief, operation_id } = st.start.clone() else { return HostOutcome::Done };
            st.start = StartState::Requesting { lane: lane.clone(), brief: brief.clone(), operation_id: operation_id.clone() };
            HostOutcome::Spawn(Job::FleetStart { operation_id, lane, brief })
        }
        "b3.fleet.dismiss" => {
            st.start_dismissed = true;
            HostOutcome::Done
        }
        "b3.fleet.providers" => HostOutcome::Action("settings.toggle".into()),
        "b3.fleet.gather" => {
            if st.gathering || !gather_admitted(store) {
                return HostOutcome::Done;
            }
            st.gathering = true;
            st.announcement = None;
            HostOutcome::Spawn(Job::FleetGather)
        }
        "b3.fleet.finished" => {
            let rows = rows(store, peers::now_ms());
            if let Some(g) = group(&rows).get(index) {
                let open = st.finished_open.entry(g.key()).or_insert(false);
                *open = !*open;
            }
            HostOutcome::Done
        }
        "b3.fleet.advanced" => {
            st.advanced_open = !st.advanced_open;
            HostOutcome::Done
        }
        "b3.fleet.approve" | "b3.fleet.deny" | "b3.fleet.stop" | "b3.fleet.steer" => {
            let Some(row) = st.drawn.get(index).cloned() else { return HostOutcome::Done };
            let steer = st.steer.get(&row.key).cloned().unwrap_or_default();
            let avail = availability(row.status, &steer);
            let (act, ok) = match action {
                "b3.fleet.approve" => (RowAction::Approve, avail.approve),
                "b3.fleet.deny" => (RowAction::Deny, avail.deny),
                "b3.fleet.stop" => (RowAction::Stop, avail.stop),
                _ => (RowAction::Steer, avail.steer),
            };
            // A disabled affordance never sends (aria-disabled + no frame).
            let Some(identity) = row.identity.clone().filter(|_| ok && row.control_supported) else {
                return HostOutcome::Done;
            };
            st.row_note.remove(&row.key);
            HostOutcome::Spawn(Job::FleetRow { key: row.key.clone(), identity, action: act, text: steer })
        }
        _ => HostOutcome::Unrouted,
    }
}

/// A text input changed (`fleet.brief`, `fleet.steer#<i>`, console fields).
pub fn input_changed(st: &mut FleetState, key: &str, text: &str) {
    if let Some(rest) = key.strip_prefix("fleet.console.") {
        super::fleet_console::input_changed(st, rest, text);
        return;
    }
    if key == "fleet.brief" {
        st.brief = text.to_owned();
        if !st.start.requesting() && st.start != StartState::Idle {
            // The operator's edit begins a NEW request.
            st.start = StartState::Idle;
            st.staged = None;
        }
        return;
    }
    if let Some(i) = key.strip_prefix("fleet.steer#").and_then(|n| n.parse::<usize>().ok()) {
        if let Some(row) = st.drawn.get(i) {
            st.steer.insert(row.key.clone(), text.to_owned());
        }
    }
}

// -------------------------------------------------------------------- view

fn status_chip(d: &mut Dsl, id: &str, status: Status) {
    let (fg, bg) = status.tone();
    d.chip(id, &format!("{} {}", status.glyph(), t(status.word())), fg, bg, None, false);
}

fn lane_summary(d: &mut Dsl, info: &LaneInfo) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 12 padding: Inset{top: 2 bottom: 2}");
    d.surface("b3_fleet_lane_avatar", "width: 34 height: 34 flow: Overlay align: Align{x: 0.5 y: 0.5}", tok::BLUE_BG, 17.0, None);
    d.text("b3_fleet_lane_initial", &info.initial(), &Txt::new(15.0, Face::Medium, tok::BLUE));
    d.close();
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 6");
    d.text("b3_fleet_lane_title", &info.title(), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
    d.chip("b3_fleet_lane_state", "Configured", tok::GREEN, tok::GREEN_BG, None, false);
    if let Some(desc) = &info.description {
        d.text("b3_fleet_lane_desc", desc, &Txt::new(12.5, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.close();
    d.close();
}

fn start_form(d: &mut Dsl, st: &FleetState, store: &Store) {
    ui::card_open(d, "b3_fleet_form", 8.0);
    ui::section_title(d, "b3_fleet_form_title", &t("Start a peer"));
    match st.lane_read {
        LaneRead::Loading => d.text("b3_fleet_loading", &t("Loading models…"), &ui::meta()),
        LaneRead::Empty => {
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 4");
            d.text(
                "b3_fleet_lanes_empty",
                &t("No peer models are configured — add one under Settings › Providers"),
                &ui::meta().w(W::Fill).wrap(),
            );
            d.link("b3_fleet_providers", &t("Settings › Providers"), Some("b3.fleet.providers"), 12.0);
            d.close();
        }
        _ => {}
    }
    let chosen = st.chosen_lane();
    if let Some(info) = chosen.as_ref().and_then(|k| st.lanes.iter().find(|l| &l.key == k)) {
        lane_summary(d, info);
    }
    ui::field_label(d, "b3_fleet_model_label", &t("Model"));
    let options = model_options(&st.lane_keys());
    let shown = chosen.as_ref().and_then(|k| options.iter().find(|(key, _)| key == k)).map(|(_, n)| n.clone());
    let ready = st.lane_read == LaneRead::Ready;
    d.surface(
        "b3_fleet_model",
        "width: Fill height: 36 flow: Overlay",
        if ready { tok::SURFACE } else { tok::SURFACE2 },
        8.0,
        Some("#d9d9dcff"),
    );
    let mrow = d.anon();
    d.view(&mrow, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 0 bottom: 0}");
    match &shown {
        Some(name) => d.text("b3_fleet_model_value", name, &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill)),
        None => d.text("b3_fleet_model_value", "", &Txt::new(12.5, Face::Regular, tok::FAINT).w(W::Fill)),
    }
    d.icon("b3_fleet_model_chevron", "chevron_down.svg", 14.0, tok::MUTED);
    d.close();
    if ready {
        d.tap("b3_fleet_model_tap", "b3.fleet.lane.toggle");
    }
    d.close();
    if st.picker_open && ready {
        d.surface("b3_fleet_options", "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 4 top: 4 bottom: 4}", tok::SURFACE, 8.0, Some(tok::HAIRLINE));
        for (i, (key, name)) in options.iter().enumerate() {
            let on = chosen.as_deref() == Some(key.as_str());
            let id = format!("b3_fleet_opt_{i}");
            d.surface(&format!("{id}_box"), "width: Fill height: 32 flow: Overlay", if on { tok::CHIP } else { tok::TRANSPARENT }, 6.0, None);
            let r = d.anon();
            d.view(&r, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 8 right: 8 top: 0 bottom: 0}");
            d.text(&format!("{id}_label"), name, &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
            d.close();
            d.tap(&id, &format!("b3.fleet.lane#{i}"));
            d.close();
        }
        d.close();
    }
    ui::field_label(d, "b3_fleet_brief_label", &t("Brief"));
    d.input("b3_fleet_brief", "fleet.brief", &st.brief_snap, "Describe the task for the peer", false, 64.0);
    match &st.start {
        StartState::Failed { kind, .. } => {
            d.text(
                "b3_fleet_error",
                &t1("Couldn't start: {value0}", fleet_driver::dispatch_refusal_label(kind)),
                &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap(),
            );
        }
        StartState::Unknown { .. } if !st.start_dismissed => {
            let row = d.anon();
            d.view(&row, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 8");
            d.text(
                "b3_fleet_unknown",
                &t("Not sure it started — Retry resends the same request."),
                &Txt::new(12.0, Face::Regular, tok::AMBER).w(W::Fill).wrap(),
            );
            d.link("b3_fleet_retry", &t("Retry"), Some("b3.fleet.retry"), 12.5);
            d.link("b3_fleet_dismiss", &t("Dismiss"), Some("b3.fleet.dismiss"), 12.5);
            d.close();
        }
        _ => {}
    }
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
    let ok = start_admitted(st, store) && !st.start.requesting();
    d.button(
        "b3_fleet_start",
        &if st.start.requesting() { t("Starting…") } else { t("Start") },
        "b3.fleet.start",
        if ok { Btn::Primary } else { Btn::Disabled },
        W::Fit,
        34.0,
    );
    d.close();
    d.close();
}

/// One fleet row card (§4.3: title · label · status · elapsed · tokens ·
/// session; then the actions).
fn row_card(d: &mut Dsl, i: usize, r: &FleetRow, st: &FleetState, control_ready: bool, terminal: bool, inner_w: f64) {
    let id = format!("b3_fleet_row_{i}");
    ui::card_open(d, &id, 8.0);
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    let av = format!("{id}_avatar");
    d.surface(&av, "width: 26 height: 26 flow: Overlay align: Align{x: 0.5 y: 0.5}", "#eef2fdff", 13.0, None);
    let initial: String = r.label.split(" · ").nth(1).and_then(|m| m.chars().next()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "P".into());
    d.text(&format!("{av}_initial"), &initial, &Txt::new(11.0, Face::Semibold, tok::BLUE));
    d.close();
    d.text(&format!("{id}_label"), &super::inventory::fit(&r.label, inner_w - 230.0, 12.5, true), &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
    status_chip(d, &format!("{id}_status"), r.status);
    d.close();
    d.text(&format!("{id}_title"), &r.title, &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill).wrap());
    let mut meta = crate::screens::dialog::minute_granularity(&peers::format_elapsed(r.elapsed_ms));
    meta.push_str(&if r.tokens > 0 { format!(" · {}", peers::format_tokens(r.tokens)) } else { " · —".to_owned() });
    if !r.session_name.is_empty() {
        meta.push_str(&format!(" · {}", r.session_name));
    }
    d.text(&format!("{id}_meta"), &meta, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill));
    // The acknowledgment (never an outcome), the replacement-turn marker,
    // or the last action's bounded copy.
    let note = if r.turn_changed {
        Some((t("Peer started a new turn"), tok::AMBER))
    } else if let Some(n) = st.row_note.get(&r.key) {
        let refused = matches!(r.control, Some(RowControl::Refused { .. })) || !matches!(n.as_str(), "Sent" | "Stop requested");
        Some((t(n), if refused { tok::RED } else { tok::GREEN }))
    } else {
        r.ack.map(|a| (t(if a == Ack::StopRequested { "Stop requested" } else { "Sent" }), tok::GREEN))
    };
    if let Some((text, color)) = note {
        d.text(&format!("{id}_note"), &text, &Txt::new(12.0, Face::Medium, color).w(W::Fill).wrap());
    }
    if terminal {
        d.close();
        return;
    }
    if !(control_ready && r.control_supported) {
        d.text(
            &format!("{id}_unsupported"),
            &t("This server does not support remote control of peers"),
            &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
        d.close();
        return;
    }
    let steer = st.steer.get(&r.key).cloned().unwrap_or_default();
    let avail = availability(r.status, &steer);
    let acts = d.anon();
    d.view(&acts, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 8");
    if r.status == Status::WaitingApproval {
        d.button(&format!("{id}_approve"), &t("Approve"), &format!("b3.fleet.approve#{i}"), Btn::Primary, W::Fit, 32.0);
        d.button(&format!("{id}_deny"), &t("Deny"), &format!("b3.fleet.deny#{i}"), Btn::Outline, W::Fit, 32.0);
    }
    d.button(
        &format!("{id}_stop"),
        &t("Stop"),
        &format!("b3.fleet.stop#{i}"),
        if avail.stop { Btn::Outline } else { Btn::OutlineOff },
        W::Fit,
        32.0,
    );
    if !avail.stop {
        d.text(&format!("{id}_stop_reason"), &t("Only while the peer is running"), &Txt::new(11.5, Face::Regular, tok::FAINT));
    }
    d.close();
    let srow = d.anon();
    d.view(&srow, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    let snap = st.steer_snap.get(&r.key).cloned().unwrap_or_default();
    d.input(&format!("{id}_steer"), &format!("fleet.steer#{i}"), &snap, &t("Enter steering text"), false, 34.0);
    let working = r.status == Status::Working;
    d.button(&format!("{id}_steer_btn"), &t("Steer"), &format!("b3.fleet.steer#{i}"), if working { Btn::Outline } else { Btn::OutlineOff }, W::Fit, 34.0);
    d.close();
    if !working {
        d.text(&format!("{id}_steer_reason"), &t("Only while working"), &Txt::new(11.5, Face::Regular, tok::FAINT));
    }
    d.close();
}

pub fn build(d: &mut Dsl, st: &mut FleetState, frame: &Frame, store: &Store) {
    // The pane replaces the chat area: a full-height panel over the
    // conversation column, its content a centred max-720 column
    // (`styles.css:433-450`).
    let x0 = st.content_x.max(0.0);
    let avail = (frame.avail_w - x0).max(280.0);
    let col_w = (avail - 32.0).clamp(240.0, 720.0);
    let inner_w = col_w;
    let now = peers::now_ms();
    let list = rows(store, now);
    st.drawn = list.clone();
    let groups = group(&list);
    let control_ready = fleet_driver::control_ready(store);
    let advertised = fleet_driver::control_advertised(store);
    d.view("b3_root", "width: Fill height: Fill flow: Right");
    if x0 > 0.0 {
        d.gap(W::Px(x0), 1.0);
    }
    d.surface("b3_fleet_panel", "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0}", tok::SURFACE, 0.0, None);
    d.open("b3_scroll", "ScrollYView", "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0} padding: Inset{left: 16 right: 16 top: 16 bottom: 24}");
    d.view("b3_fleet_col", &format!("width: {} height: Fit flow: Down spacing: 12", col_w.floor()));
    // Header: Back + "Fleet" + the empty-state word on the right.
    let head = d.anon();
    d.view(&head, "width: Fill height: 34 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    d.button("b3_fleet_back", "Back", "b3.close", Btn::Outline, W::Fit, 30.0);
    d.text("b3_title", &t("Fleet"), &ui::title().w(W::Fill));
    let session_open = store.domains.session.active().is_some();
    if list.is_empty() && session_open {
        d.text("b3_fleet_none", &t("No peers yet"), &ui::meta());
    }
    // The blackboard gather (`/gather`): the synthesis rides one turn.
    if session_open && gather_admitted(store) {
        let label = if st.gathering { t("Gathering…") } else { t("Peer gather") };
        d.button("b3_fleet_gather", &label, "b3.fleet.gather", if st.gathering { Btn::Disabled } else { Btn::Outline }, W::Fit, 30.0);
    }
    d.close();
    if let Some(a) = &st.announcement {
        d.text("b3_fleet_announce", a, &Txt::new(12.0, Face::Medium, tok::BLUE).w(W::Fill).wrap());
    }
    if !session_open {
        d.text("b3_fleet_noproject", &t("Open a project first"), &Txt::new(14.0, Face::Medium, tok::MUTED));
    } else {
        if !advertised {
            d.text("b3_fleet_unsupported", &t("This server does not support starting peers"), &ui::meta().w(W::Fill).wrap());
        } else if !control_ready {
            d.text("b3_fleet_notready", &t("Peer controls are not ready"), &ui::meta().w(W::Fill).wrap());
        } else {
            start_form(d, st, store);
        }
        for (gi, g) in groups.iter().enumerate() {
            let heading = match &g.goal_id {
                Some(goal) => t1("Goal {value0}", goal),
                None => t("Peers"),
            };
            d.text(&format!("b3_fleet_group_{gi}"), &heading, &Txt::new(12.0, Face::Semibold, tok::MUTED).w(W::Fill));
            for &i in &g.active {
                row_card(d, i, &list[i], st, control_ready, false, inner_w);
            }
            if !g.finished.is_empty() {
                let open = st.finished_open.get(&g.key()).copied().unwrap_or(false);
                d.view(&format!("b3_fleet_finished_{gi}_head"), "width: Fill height: 30 flow: Overlay");
                let h = d.anon();
                d.view(&h, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
                d.icon("", if open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" }, 13.0, tok::TEXT);
                d.text(&format!("b3_fleet_finished_{gi}_label"), &t1("Finished ({value0})", &g.finished.len().to_string()), &Txt::new(13.0, Face::Regular, tok::TEXT));
                d.close();
                d.tap(&format!("b3_fleet_finished_{gi}"), &format!("b3.fleet.finished#{gi}"));
                d.close();
                if open {
                    for &i in &g.finished {
                        row_card(d, i, &list[i], st, control_ready, true, inner_w);
                    }
                }
            }
        }
        // §4.3 footer: the session controller behind Advanced (closed by
        // default), only when control is supported.
        if control_ready {
            d.view("b3_fleet_advanced_head", "width: Fill height: 32 flow: Overlay");
            let h = d.anon();
            d.view(&h, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
            d.icon("", if st.advanced_open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" }, 13.0, tok::TEXT);
            d.text("b3_fleet_advanced_label", &t("Advanced"), &Txt::new(13.0, Face::Medium, tok::TEXT));
            d.close();
            d.tap("b3_fleet_advanced", "b3.fleet.advanced");
            d.close();
            if st.advanced_open {
                super::fleet_console::build(d, st, store, inner_w);
            }
        }
    }
    d.close(); // col
    d.close(); // scroll
    d.close(); // panel
    d.close(); // root
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_store::domains::peer::{InventoryOp, Origin, PeerSessionEvent};

    fn op(id: &str, slug: &str, goal: Option<&str>) -> InventoryOp {
        InventoryOp {
            operation_id: id.into(),
            slug: slug.into(),
            lifecycle: "started".into(),
            adopted_session_id: format!("dsflash:main#peer-{slug}"),
            adopted_turn_id: "00000000-0000-4000-8000-0000000000d1".into(),
            workspace_root: "/srv/work/octos".into(),
            model: "gpt-5.4".into(),
            model_lane: "lane-primary".into(),
            goal_id: goal.map(Into::into),
            accepted_at_ms: 1_000,
        }
    }

    fn inventory(store: &Store, ops: Vec<InventoryOp>) {
        store.domains.peer.set_inventory(Some(FleetInventory::Complete {
            session_id: "dsflash:main".into(),
            snapshot: "s".into(),
            observed_revision: "42".into(),
            operations: ops,
            disclosure: octoscode_store::domains::peer::Disclosure {
                mode: "external".into(),
                recovery: "none".into(),
                binding: None,
            },
            completed_at_ms: 0,
        }));
    }

    #[test]
    fn the_status_words_follow_the_rows_own_events() {
        let mut r = PeerRow::opening("m#peer-a", "a", Origin::Dispatch, "t", 1_000);
        assert_eq!(row_status(&r, 2_000), Status::Starting);
        assert_eq!(row_status(&r, 1_000 + SLOW_START_MS + 1), Status::StillStarting, "15 s slow start");
        r.status = RowStatus::Started;
        assert_eq!(row_status(&r, 0), Status::Working);
        r.activity = Activity::Blocked;
        r.request_kind = Some(RequestKind::Question);
        assert_eq!(row_status(&r, 0), Status::WaitingAnswer);
        r.request_kind = Some(RequestKind::Approval);
        assert_eq!(row_status(&r, 0), Status::WaitingApproval);
        r.activity = Activity::Done;
        r.outcome = Some(Outcome::Stopped);
        assert_eq!(row_status(&r, 0), Status::Stopped);
        r.outcome = None;
        r.status = RowStatus::Unknown;
        assert_eq!(row_status(&r, 0), Status::Unknown);
        assert_eq!(Status::StillStarting.word(), "Still starting…");
    }

    #[test]
    fn the_union_keys_by_adopted_session_and_labels_never_show_a_slug() {
        let store = Store::new();
        store.set_active(Some("dsflash:main".into()));
        inventory(&store, vec![op("op-a", "tests-1a2b", None)]);
        // The roster row that IS the inventory's adopted session merges.
        let mut adopted = PeerRow::opening("dsflash:main#peer-tests-1a2b", "tests-1a2b", Origin::Dispatch, "t", 0);
        adopted.status = RowStatus::Started;
        adopted.operation_id = Some("op-a".into());
        adopted.brief = "Run the test suite\nand report".into();
        store.domains.peer.stage_row(adopted, false);
        // A prepare-only roster row stays its own row.
        store.domains.peer.stage_row(PeerRow::opening("dsflash:local:tui#peer-docs", "docs", Origin::Staged, "t", 0), false);
        let r = rows(&store, 5_000);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].label, "Peer 1 · gpt-5.4");
        assert_eq!(r[0].title, "Run the test suite");
        assert_eq!(r[0].status, Status::Working);
        assert_eq!(r[0].elapsed_ms, 4_000, "elapsed from accepted_at_ms");
        assert_eq!(r[1].label, "Peer 2");
        assert!(r.iter().all(|x| !x.label.contains("tests-1a2b") && !x.title.contains("docs")), "{r:?}");
        // Inventory-only rows read Requested.
        inventory(&store, vec![op("op-a", "tests-1a2b", None), op("op-b", "lint", None)]);
        assert_eq!(rows(&store, 5_000)[1].status, Status::Requested);
    }

    #[test]
    fn groups_follow_goal_order_with_peers_last_and_rank_stable() {
        let mk = |goal: Option<&str>, status: Status| FleetRow {
            key: format!("{goal:?}{status:?}"),
            identity: None,
            slug: String::new(),
            label: String::new(),
            title: String::new(),
            status,
            session_name: String::new(),
            goal_id: goal.map(Into::into),
            elapsed_ms: 0,
            tokens: 0,
            control_supported: false,
            ack: None,
            turn_changed: false,
            control: None,
            error: None,
        };
        let rows = vec![
            mk(None, Status::Working),
            mk(Some("g1"), Status::Finished),
            mk(Some("g1"), Status::Starting),
            mk(Some("g1"), Status::WaitingApproval),
            mk(Some("g2"), Status::Working),
        ];
        let g = group(&rows);
        assert_eq!(g.iter().map(|g| g.goal_id.clone()).collect::<Vec<_>>(), vec![Some("g1".into()), Some("g2".into()), None]);
        assert_eq!(g[0].active, vec![3, 2], "waiting first, then starting");
        assert_eq!(g[0].finished, vec![1], "terminal rows under Finished (n)");
        assert_eq!(group(&rows[..1])[0].goal_id, None, "one flat Peers group");
    }

    #[test]
    fn availability_and_announcements_are_the_design_table() {
        assert_eq!(
            availability(Status::WaitingApproval, ""),
            Availability { approve: true, deny: true, steer: false, stop: true }
        );
        assert!(availability(Status::Working, "go").steer && !availability(Status::Working, "  ").steer);
        assert!(!availability(Status::Finished, "x").stop && availability(Status::StillStarting, "").stop);
        let row = |key: &str, s: Status| FleetRow {
            key: key.into(),
            identity: None,
            slug: String::new(),
            label: format!("Peer {key}"),
            title: String::new(),
            status: s,
            session_name: String::new(),
            goal_id: None,
            elapsed_ms: 0,
            tokens: 0,
            control_supported: false,
            ack: None,
            turn_changed: false,
            control: None,
            error: None,
        };
        let now = vec![row("1", Status::WaitingApproval)];
        assert_eq!(announce(&[], &now).as_deref(), Some("Peer 1 is waiting for your approval"));
        assert_eq!(announce(&[("1".into(), Status::WaitingApproval)], &now), None, "silent when nothing changed");
        assert_eq!(announce(&[], &[row("2", Status::Finished)]).as_deref(), Some("Peer 2 finished"));
        assert_eq!(announce(&[], &[row("3", Status::Working)]), None);
    }

    #[test]
    fn the_picker_reads_only_advertised_keys_without_a_default() {
        assert_eq!(model_name("glm-53"), "glm-5.3");
        assert_eq!(model_name("qwen-coder-32"), "qwen-coder-32", "only name-dd is unfolded");
        assert_eq!(model_name("lane-primary"), "lane-primary");
        let opts = model_options(&["glm-53".into(), "glm-5.3".into(), "lane-review".into()]);
        assert_eq!(opts[0].1, "glm-5.3 (glm-53)");
        assert_eq!(opts[2].1, "lane-review");
        let mut st = FleetState { lanes: vec![LaneInfo { key: "lane-primary".into(), ..Default::default() }], ..Default::default() };
        assert_eq!(st.chosen_lane(), None, "no implicit default");
        st.lane = "lane-primary".into();
        assert_eq!(st.chosen_lane().as_deref(), Some("lane-primary"));
        st.lanes = vec![LaneInfo { key: "lane-review".into(), ..Default::default() }];
        assert_eq!(st.chosen_lane(), None, "a withdrawn lane collapses to none");
    }

    #[test]
    fn a_blocked_row_folded_from_events_offers_approve_and_deny() {
        let store = Store::new();
        store.set_active(Some("dsflash:main".into()));
        let mut r = PeerRow::opening("dsflash:main#peer-a", "a", Origin::Dispatch, "t", 0);
        r.status = RowStatus::Started;
        r.operation_id = Some("op".into());
        store.domains.peer.stage_row(r, false);
        store.domains.peer.observe_session_event(
            "dsflash:main#peer-a",
            &PeerSessionEvent::AttentionRequested { request_id: Some("ap".into()), kind: Some(RequestKind::Approval), detail: None },
            1,
        );
        let rows = rows(&store, 2);
        assert_eq!(rows[0].status, Status::WaitingApproval);
        assert!(rows[0].control_supported);
    }
}
