//! Board-3 screen 4 — FLEET destination + Start form (rows: fleet × 6).
//!
//! Web: `features/fleet/FleetView.tsx` (`:228-470`), reached from the sidebar
//! footer's "Fleet" entry (`fleet-navigation.ts:21-25`, `ProductSidebar.tsx:
//! 970-983`); the pane REPLACES the chat area and its only way out is "Back"
//! (`App.tsx:3048-3136`). Model:
//! * status words — FleetPane's production mapping (`FleetPane.tsx:163-182`):
//!   no roster entry -> Requested, opening -> Starting, started -> Working,
//!   blocked -> Waiting for your approval, closed/finished -> Finished,
//!   stopped -> Stopped, failed -> Failed, else Outcome unknown;
//! * grouping/order (`fleet-model.ts:219-256`): one group per goal id (the
//!   goal-less "Peers" group last), stable rank order approval < answer <
//!   Working < Starting|Requested < Finished < Stopped < Failed, terminal
//!   rows collapsed under "Finished (n)" (closed by default);
//! * labels (`fleet-facts.ts:218-220,299-304`): `Peer N · model`, N counting
//!   our own dispatches first, then roster-only peers ("Peer N"); a slug is
//!   never shown;
//! * announcements (`fleet-model.ts:442-459`): a peer that starts waiting or
//!   finishes/stops/fails is announced once; nothing changed -> silent;
//! * Start (`App.tsx` fleet-start-sequencer + `external-driver*.ts`):
//!   `peer/prepare {brief, title, session_id, profile_id}` -> the driver seat
//!   `session/driver/acquire` -> EXACTLY ONE `peer/dispatch` (snake_case wire,
//!   `external-driver-peer-control.ts:632-644`), gated on the advertised
//!   `peer/dispatch` + `peer/control` methods and `external_driver_v1`;
//! * steering (`FleetView.tsx:661-745`): a one-line "Enter steering text" +
//!   "Steer", enabled ONLY while the row is Working ("Only while working").
//!   (The board's slider/"Dismiss" are its known image flaw — not copied.)
use octoscode_store::Store;

use super::host::Outcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// The peer phases the web's FleetPane distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Requested,
    Starting,
    Working,
    WaitingApproval,
    WaitingAnswer,
    Finished,
    Stopped,
    Failed,
    Unknown,
}

impl Phase {
    /// `FleetPane.tsx:163-182` status word.
    pub fn word(self) -> &'static str {
        match self {
            Phase::Requested => "Requested",
            Phase::Starting => "Starting",
            Phase::Working => "Working",
            Phase::WaitingApproval => "Waiting for your approval",
            Phase::WaitingAnswer => "Waiting for your answer",
            Phase::Finished => "Finished",
            Phase::Stopped => "Stopped",
            Phase::Failed => "Failed",
            Phase::Unknown => "Outcome unknown",
        }
    }
    /// The sort rank (`fleet-model.ts:219-256`).
    pub fn rank(self) -> u8 {
        match self {
            Phase::WaitingApproval => 0,
            Phase::WaitingAnswer => 1,
            Phase::Working => 2,
            Phase::Starting | Phase::Requested => 3,
            Phase::Finished => 4,
            Phase::Stopped => 5,
            Phase::Failed | Phase::Unknown => 6,
        }
    }
    pub fn terminal(self) -> bool {
        matches!(self, Phase::Finished | Phase::Stopped | Phase::Failed | Phase::Unknown)
    }
    /// The board's avatar dot.
    pub fn dot(self) -> &'static str {
        match self {
            Phase::WaitingApproval | Phase::WaitingAnswer => "#e8a33cff",
            Phase::Working => tok::GREEN,
            Phase::Starting | Phase::Requested => tok::BLUE,
            Phase::Finished => tok::FAINT,
            Phase::Stopped | Phase::Failed | Phase::Unknown => tok::RED,
        }
    }
}

/// One of OUR dispatches (the inventory acceptance facts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    pub operation_id: String,
    pub model: String,
    pub brief: String,
    pub slug: Option<String>,
    pub phase: Phase,
    pub error: Option<String>,
    /// The driver fence the dispatch rode (steering reuses it).
    pub fence: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub phase: Phase,
    pub brief: String,
    pub op: Option<usize>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FleetState {
    pub lanes: Vec<String>,
    /// The lanes' reported facts (provider, model, description), same order.
    pub lane_info: Vec<LaneInfo>,
    pub lanes_loading: bool,
    pub lane: usize,
    pub brief: String,
    pub brief_snap: String,
    pub steer: String,
    pub steer_snap: String,
    pub ops: Vec<Op>,
    pub start_error: Option<String>,
    pub starting: bool,
    pub finished_open: bool,
    pub announcement: Option<String>,
    /// label -> last phase, for the announcement diff.
    pub seen: Vec<(String, Phase)>,
    pub content_x: f64,
}

/// One `profile/sub_providers/list` row as the Start form summarises it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneInfo {
    pub key: String,
    pub provider: String,
    pub model: Option<String>,
    pub description: Option<String>,
}

impl LaneInfo {
    /// `provider/model` when both are reported, else the lane key.
    pub fn title(&self) -> String {
        match (&self.model, self.provider.trim().is_empty()) {
            (Some(m), false) if !m.trim().is_empty() => format!("{}/{}", self.provider.trim(), m.trim()),
            _ => model_label(&self.key),
        }
    }
    /// The avatar's initial (the provider's, else the key's).
    pub fn initial(&self) -> String {
        let src = if self.provider.trim().is_empty() { &self.key } else { &self.provider };
        src.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into())
    }
}

/// `peer-row-view` model label: `glm-53` -> `glm-5.3` (a digit pair after a
/// dash reads as a version).
pub fn model_label(key: &str) -> String {
    let mut out = String::new();
    for (i, part) in key.split('-').enumerate() {
        if i > 0 {
            out.push('-');
        }
        if part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit()) && i > 0 {
            out.push_str(&format!("{}.{}", &part[..1], &part[1..]));
        } else {
            out.push_str(part);
        }
    }
    out
}

/// The roster ∪ our dispatches, labelled and ordered (`fleet-facts.ts`).
pub fn rows(st: &FleetState, store: &Store) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let roster = store.domains.peer.list();
    let mut n = 0usize;
    for (i, op) in st.ops.iter().enumerate() {
        n += 1;
        // A roster entry for our slug supersedes the local phase only when it
        // has closed (the roster carries open/closed).
        let phase = match op.slug.as_ref().and_then(|s| roster.iter().find(|p| &p.name == s)) {
            Some(p) if p.closed => Phase::Finished,
            _ => op.phase,
        };
        out.push(Row {
            label: format!("Peer {n} · {}", model_label(&op.model)),
            phase,
            brief: op.brief.clone(),
            op: Some(i),
            slug: op.slug.clone(),
        });
    }
    let mut extra: Vec<_> = roster
        .iter()
        .filter(|p| !st.ops.iter().any(|o| o.slug.as_deref() == Some(p.name.as_str())))
        .collect();
    extra.sort_by(|a, b| a.staged_at_ms.cmp(&b.staged_at_ms).then(a.name.cmp(&b.name)));
    for p in extra {
        n += 1;
        out.push(Row {
            label: format!("Peer {n}"),
            phase: if p.closed { Phase::Finished } else { Phase::Working },
            brief: p.topic.clone().unwrap_or_default(),
            op: None,
            slug: Some(p.name.clone()),
        });
    }
    // Stable sort by rank.
    out.sort_by_key(|r| r.phase.rank());
    out
}

/// `fleet-model.ts:442-459`: the first changed row into an announced state.
pub fn announce(prev: &[(String, Phase)], rows: &[Row]) -> Option<String> {
    for r in rows {
        let before = prev.iter().find(|(l, _)| l == &r.label).map(|(_, p)| *p);
        if before == Some(r.phase) {
            continue;
        }
        let msg = match r.phase {
            Phase::WaitingApproval => format!("{} is waiting for your approval", r.label),
            Phase::WaitingAnswer => format!("{} is waiting for your answer", r.label),
            Phase::Finished => format!("{} finished", r.label),
            Phase::Stopped => format!("{} stopped", r.label),
            Phase::Failed => format!("{} failed", r.label),
            _ => continue,
        };
        return Some(msg);
    }
    None
}

/// The Start gate (`fleet-actions.ts`, `peer-dispatch-commands.ts:47`).
pub fn start_supported(store: &Store) -> bool {
    let m = store.domains.config.supported_methods();
    m.iter().any(|x| x == "peer/dispatch")
        && m.iter().any(|x| x == "peer/control")
        && store.domains.config.has_capability("external_driver_v1")
}

/// `peer-manager.ts:45-47` — the kickoff prompt.
pub fn kickoff_prompt(brief: &str, brief_path: &str) -> String {
    format!("You are a peer agent. Your brief:\n\n{brief}\n\n(The durable copy of this brief is at {brief_path} — re-read it if your context is compacted.)")
}

fn mint_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    format!("{prefix}-{}-{}", ui::now_ms(), SEQ.fetch_add(1, Ordering::Relaxed))
}

// --------------------------------------------------------------- transport

pub async fn load_lanes(conv: &crate::flow::Conversation) -> Result<String, String> {
    use octoscode_client::domains::profile::{SubProvidersList, SubProvidersListParams};
    super::host::state().fleet.lanes_loading = true;
    let r = conv
        .client()
        .call::<SubProvidersList>(SubProvidersListParams { profile_id: Some(conv.profile()) })
        .await;
    let mut st = super::host::state();
    st.fleet.lanes_loading = false;
    match r {
        Ok(v) => {
            st.fleet.lane_info = v
                .sub_providers
                .iter()
                .map(|l| LaneInfo {
                    key: l.key.clone(),
                    provider: l.provider.clone(),
                    model: l.model.clone(),
                    description: l.description.clone().filter(|d| !d.trim().is_empty()),
                })
                .collect();
            st.fleet.lanes = v.sub_providers.into_iter().map(|l| l.key).collect();
            Ok(format!("{} lanes", st.fleet.lanes.len()))
        }
        Err(e) => Err(e.to_string()),
    }
}

/// The Start chain: prepare -> driver seat -> ONE dispatch. A refusal keeps
/// the brief for retry; success clears it.
pub async fn start(conv: &crate::flow::Conversation, model: String, brief: String) -> Result<String, String> {
    use octoscode_client::domains::peer::{PeerDispatch, PeerPrepare, PeerPrepareParams};
    use octoscode_client::domains::session::SessionDriverAcquire;
    let session = conv.session_id();
    let profile = conv.profile();
    let op_id = mint_id("op");
    let title: String = brief.lines().next().unwrap_or("").chars().take(60).collect();
    let idx = {
        let mut st = super::host::state();
        st.fleet.ops.push(Op {
            operation_id: op_id.clone(),
            model: model.clone(),
            brief: brief.clone(),
            slug: None,
            phase: Phase::Requested,
            error: None,
            fence: None,
        });
        st.fleet.ops.len() - 1
    };
    let fail = |e: String| {
        let mut st = super::host::state();
        st.fleet.starting = false;
        st.fleet.start_error = Some(format!("Couldn't start: {}", "Couldn't start that peer."));
        if let Some(op) = st.fleet.ops.get_mut(idx) {
            op.phase = Phase::Failed;
            op.error = Some(e.clone());
        }
        super::host::wake();
        Err(e)
    };
    let prepared = match conv
        .client()
        .call::<PeerPrepare>(PeerPrepareParams {
            brief: brief.trim().to_owned(),
            n: None,
            title: (!title.is_empty()).then_some(title),
            names: None,
            worktree: None,
            cwd: None,
            session_id: session.clone(),
            profile_id: profile.clone(),
        })
        .await
    {
        Ok(p) => p,
        Err(e) => return fail(e.to_string()),
    };
    {
        let mut st = super::host::state();
        if let Some(op) = st.fleet.ops.get_mut(idx) {
            op.slug = Some(prepared.slug.clone());
            op.phase = Phase::Starting;
        }
    }
    super::host::wake();
    let driver_id = mint_id("octoscode-native");
    let fence = match conv
        .client()
        .call::<SessionDriverAcquire>(serde_json::json!({
            "session_id": session, "driver_id": driver_id, "lease_seconds": 120,
        }))
        .await
    {
        Ok(v) => v,
        Err(e) => return fail(e.to_string()),
    };
    let wire = serde_json::json!({
        "session_id": session,
        "driver_id": fence.get("driver_id").cloned().unwrap_or(serde_json::json!(driver_id)),
        "epoch": fence.get("epoch").cloned().unwrap_or(serde_json::Value::Null),
        "control_token": fence.get("control_token").cloned().unwrap_or(serde_json::Value::Null),
        "operation_id": op_id,
        "model": model,
        "dispatch": {"kind": "new_brief", "brief": brief.trim(), "title": prepared.slug},
        "kickoff_input": [{"kind": "text", "text": kickoff_prompt(brief.trim(), &prepared.brief_path)}],
    });
    match conv.client().call::<PeerDispatch>(wire).await {
        Ok(_) => {
            let mut st = super::host::state();
            st.fleet.starting = false;
            st.fleet.start_error = None;
            st.fleet.brief.clear();
            st.fleet.brief_snap.clear();
            if let Some(op) = st.fleet.ops.get_mut(idx) {
                op.phase = Phase::Working;
                op.fence = Some(fence);
            }
            Ok(format!("dispatched {op_id}"))
        }
        Err(e) => fail(e.to_string()),
    }
}

/// `peer/control` steer for one of our Working peers.
pub async fn steer(conv: &crate::flow::Conversation, op_index: usize, text: String) -> Result<String, String> {
    use octoscode_client::domains::peer::PeerControl;
    let (fence, target) = {
        let st = super::host::state();
        let Some(op) = st.fleet.ops.get(op_index) else { return Err("no such peer".into()) };
        (op.fence.clone().unwrap_or_default(), op.operation_id.clone())
    };
    let wire = serde_json::json!({
        "session_id": conv.session_id(),
        "driver_id": fence.get("driver_id"),
        "epoch": fence.get("epoch"),
        "control_token": fence.get("control_token"),
        "operation_id": mint_id("op"),
        "target_operation_id": target,
        "command": {"kind": "steer", "input": [{"kind": "text", "text": text}]},
    });
    conv.client().call::<PeerControl>(wire).await.map(|_| "steered".to_owned()).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ actions

pub fn perform(st: &mut FleetState, action: &str, index: usize, store: &Store) -> Outcome {
    match action {
        "b3.fleet.lane" => {
            if !st.lanes.is_empty() {
                st.lane = (st.lane + 1) % st.lanes.len();
            }
            st.brief_snap = st.brief.clone();
            Outcome::Done
        }
        "b3.fleet.start" => {
            if st.starting || !start_supported(store) {
                return Outcome::Done;
            }
            let Some(model) = st.lanes.get(st.lane).cloned() else {
                st.start_error = Some("No peer models are configured — add one under Settings › Providers".into());
                return Outcome::Done;
            };
            if st.brief.trim().is_empty() {
                return Outcome::Done; // the Start gate: a blank brief never starts
            }
            st.starting = true;
            st.start_error = None;
            Outcome::Spawn(super::host::Job::FleetStart(model, st.brief.clone()))
        }
        "b3.fleet.steer" => {
            let rows = rows(st, store);
            let Some(row) = rows.get(index) else { return Outcome::Done };
            match (row.phase, row.op) {
                (Phase::Working, Some(op)) if !st.steer.trim().is_empty() => {
                    Outcome::Spawn(super::host::Job::FleetSteer(op, st.steer.clone()))
                }
                _ => Outcome::Done, // "Only while working" — never sends
            }
        }
        "b3.fleet.finished" => {
            st.finished_open = !st.finished_open;
            st.brief_snap = st.brief.clone();
            Outcome::Done
        }
        _ => Outcome::Unrouted,
    }
}

pub fn input_changed(st: &mut FleetState, key: &str, text: &str) {
    match key {
        "fleet.brief" => st.brief = text.to_owned(),
        "fleet.steer" => st.steer = text.to_owned(),
        _ => {}
    }
}

// -------------------------------------------------------------------- view

fn lane_summary(d: &mut Dsl, info: &LaneInfo) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.0} spacing: 12 padding: Inset{top: 2 bottom: 6}");
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

fn peer_card(d: &mut Dsl, i: usize, r: &Row, inner_w: f64) {
    let id = format!("b3_fleet_row_{i}");
    ui::card_open(d, &id, 8.0);
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    // The avatar dot with the peer's initial.
    let av = format!("{id}_avatar");
    d.surface(&av, "width: 26 height: 26 flow: Overlay align: Align{x: 0.5 y: 0.5}", "#eef2fdff", 13.0, None);
    d.text("", &r.label.chars().last().map(|c| c.to_uppercase().to_string()).unwrap_or_default(), &Txt::new(11.0, Face::Semibold, tok::BLUE));
    d.close();
    d.text(&format!("{id}_label"), &super::inventory::fit(&r.label, inner_w - 240.0, 12.5, true), &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
    let (fg, bg) = match r.phase {
        Phase::WaitingApproval | Phase::WaitingAnswer => (tok::AMBER, tok::AMBER_BG),
        Phase::Working => (tok::GREEN, tok::GREEN_BG),
        Phase::Failed | Phase::Stopped | Phase::Unknown => (tok::RED, tok::RED_BG),
        _ => (tok::MUTED, tok::SURFACE2),
    };
    d.dot(r.phase.dot(), 7.0);
    d.chip(&format!("{id}_status"), r.phase.word(), fg, bg, None, false);
    d.close();
    if !r.brief.is_empty() {
        d.text(&format!("{id}_brief"), &super::inventory::fit(&r.brief, (inner_w - 40.0) * 2.0, 12.0, false), &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    if !r.phase.terminal() {
        let srow = d.anon();
        d.view(&srow, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.input(&format!("{id}_steer"), "fleet.steer", "", "Enter steering text", false, 34.0);
        let live = r.phase == Phase::Working && r.op.is_some();
        d.button(&format!("{id}_steer_btn"), "Steer", &format!("b3.fleet.steer#{i}"), if live { Btn::Outline } else { Btn::Disabled }, W::Fit, 34.0);
        d.close();
        if r.phase != Phase::Working {
            d.text("", "Only while working", &Txt::new(11.5, Face::Regular, tok::FAINT));
        }
    }
    d.close();
}

pub fn build(d: &mut Dsl, st: &FleetState, frame: &Frame, store: &Store) {
    // The pane replaces the chat area: a full-height panel over the
    // conversation column, its content a centred max-720 column
    // (`styles.css:433-450`).
    let x0 = st.content_x.max(0.0);
    let avail = (frame.avail_w - x0).max(280.0);
    let col_w = (avail - 32.0).clamp(240.0, 720.0);
    let inner_w = col_w;
    d.view("b3_root", "width: Fill height: Fill flow: Right");
    if x0 > 0.0 {
        d.gap(W::Px(x0), 1.0);
    }
    d.surface("b3_fleet_panel", "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0}", tok::SURFACE, 0.0, None);
    d.open("b3_scroll", "ScrollYView", "width: Fill height: Fill flow: Down align: Align{x: 0.5 y: 0.0} padding: Inset{left: 16 right: 16 top: 16 bottom: 16}");
    d.view("b3_fleet_col", &format!("width: {} height: Fit flow: Down spacing: 12", col_w.floor()));
    // Header: Back + "Fleet" + the empty-state word on the right.
    let head = d.anon();
    d.view(&head, "width: Fill height: 34 flow: Right align: Align{x: 0.0 y: 0.5} spacing: 10");
    d.button("b3_fleet_back", "Back", "b3.close", Btn::Outline, W::Fit, 30.0);
    d.text("b3_title", "Fleet", &ui::title().w(W::Fill));
    let list = rows(st, store);
    if list.is_empty() {
        d.text("b3_fleet_none", "No peers yet", &ui::meta());
    }
    d.close();
    if let Some(a) = &st.announcement {
        d.text("b3_fleet_announce", a, &Txt::new(12.0, Face::Regular, tok::BLUE).w(W::Fill));
    }
    let session_open = store.domains.session.active().is_some();
    if !session_open {
        d.text("b3_fleet_noproject", "Open a project first", &Txt::new(14.0, Face::Medium, tok::MUTED));
        d.text("", "Peers run inside a session. Choose a workspace to continue.", &ui::meta().w(W::Fill));
    } else {
        // The Start form card.
        ui::card_open(d, "b3_fleet_form", 8.0);
        ui::section_title(d, "b3_fleet_form_title", "Start a peer");
        if !start_supported(store) {
            d.text("b3_fleet_unsupported", "This server does not support starting peers", &ui::meta().w(W::Fill));
        }
        if st.lanes_loading {
            d.text("b3_fleet_loading", "Loading models…", &ui::meta());
        }
        // The board's lane summary: who runs the peer, from the server's
        // own `sub_providers` row (initial, provider/model, description).
        if let Some(info) = st.lane_info.get(st.lane) {
            lane_summary(d, info);
        }
        ui::field_label(d, "", "Model");
        let lane = st.lanes.get(st.lane).map(|l| model_label(l));
        d.surface("b3_fleet_model", "width: Fill height: 36 flow: Overlay", tok::SURFACE, 8.0, Some("#d9d9dcff"));
        let mrow = d.anon();
        d.view(&mrow, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 0 bottom: 0}");
        d.text(
            "b3_fleet_model_value",
            lane.as_deref().unwrap_or("No peer models are configured — add one under Settings › Providers"),
            &Txt::new(12.5, if lane.is_some() { Face::Mono } else { Face::Regular }, if lane.is_some() { tok::TEXT } else { tok::MUTED }).w(W::Fill),
        );
        d.icon("", "chevron_down.svg", 14.0, tok::MUTED);
        d.close();
        if st.lanes.len() > 1 {
            d.tap("b3_fleet_model_tap", "b3.fleet.lane");
        }
        d.close();
        ui::field_label(d, "", "Brief");
        d.input("b3_fleet_brief", "fleet.brief", &st.brief_snap, "Describe the task for the peer", false, 64.0);
        if let Some(e) = &st.start_error {
            d.text("b3_fleet_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
        }
        let foot = d.anon();
        d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5}");
        let ok = start_supported(store) && lane.is_some() && !st.starting;
        d.button("b3_fleet_start", if st.starting { "Starting…" } else { "Start" }, "b3.fleet.start", if ok { Btn::Primary } else { Btn::Disabled }, W::Fit, 34.0);
        d.close();
        d.close();
        // The roster: live rows, then the collapsed "Finished (n)".
        let (live, done): (Vec<(usize, &Row)>, Vec<(usize, &Row)>) =
            list.iter().enumerate().partition(|(_, r)| !r.phase.terminal());
        if !list.is_empty() {
            d.text("", "Peers", &Txt::new(11.0, Face::Semibold, tok::MUTED));
        }
        for (i, r) in live {
            peer_card(d, i, r, inner_w);
        }
        if !done.is_empty() {
            d.view("b3_fleet_finished_head", "width: Fill height: 30 flow: Overlay");
            let h = d.anon();
            d.view(&h, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
            d.icon("", if st.finished_open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" }, 13.0, tok::TEXT);
            d.text("b3_fleet_finished_label", &format!("Finished ({})", done.len()), &Txt::new(13.0, Face::Regular, tok::TEXT));
            d.close();
            d.tap("b3_fleet_finished_tap", "b3.fleet.finished");
            d.close();
            if st.finished_open {
                for (i, r) in done {
                    peer_card(d, i, r, inner_w);
                }
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
    use octoscode_store::domains::peer::Peer;

    fn op(model: &str, phase: Phase, slug: Option<&str>) -> Op {
        Op { operation_id: "op".into(), model: model.into(), brief: "b".into(), slug: slug.map(Into::into), phase, error: None, fence: None }
    }

    #[test]
    fn labels_count_our_dispatches_first_and_never_show_a_slug() {
        let store = Store::new();
        let mut p = Peer::named("tests-1a2b");
        p.staged_at_ms = 5;
        store.domains.peer.upsert(p);
        let st = FleetState { ops: vec![op("glm-53", Phase::Starting, Some("mine"))], ..Default::default() };
        let r = rows(&st, &store);
        assert_eq!(r.len(), 2);
        assert!(r.iter().any(|x| x.label == "Peer 1 · glm-5.3"));
        assert!(r.iter().any(|x| x.label == "Peer 2"), "roster-only peer has no model");
        assert!(!r.iter().any(|x| x.label.contains("tests-1a2b")), "slugs are never shown");
    }

    #[test]
    fn rows_order_by_the_web_rank_and_terminal_collapses() {
        let store = Store::new();
        let st = FleetState {
            ops: vec![
                op("a", Phase::Finished, None),
                op("b", Phase::Working, None),
                op("c", Phase::WaitingApproval, None),
                op("d", Phase::Requested, None),
            ],
            ..Default::default()
        };
        let words: Vec<&str> = rows(&st, &store).iter().map(|r| r.phase.word()).collect();
        assert_eq!(words, ["Waiting for your approval", "Working", "Requested", "Finished"]);
        assert!(Phase::Finished.terminal() && !Phase::Working.terminal());
    }

    #[test]
    fn announcements_fire_once_per_change_and_stay_silent_otherwise() {
        let store = Store::new();
        let st = FleetState { ops: vec![op("m", Phase::Finished, None)], ..Default::default() };
        let r = rows(&st, &store);
        assert_eq!(announce(&[], &r).as_deref(), Some("Peer 1 · m finished"));
        let seen: Vec<(String, Phase)> = r.iter().map(|x| (x.label.clone(), x.phase)).collect();
        assert_eq!(announce(&seen, &r), None, "nothing changed -> silent");
    }

    #[test]
    fn start_is_gated_and_a_blank_brief_never_starts() {
        let store = Store::new();
        let mut st = FleetState { lanes: vec!["glm-53".into()], ..Default::default() };
        st.brief = "Review the diff".into();
        assert_eq!(perform(&mut st, "b3.fleet.start", 0, &store), Outcome::Done, "unsupported server");
        store.domains.config.set_supported_methods(vec!["peer/dispatch".into(), "peer/control".into()]);
        store.set_capabilities(vec!["external_driver_v1".into()]);
        st.brief = "   ".into();
        assert_eq!(perform(&mut st, "b3.fleet.start", 0, &store), Outcome::Done);
        st.brief = "Review the diff".into();
        assert_eq!(
            perform(&mut st, "b3.fleet.start", 0, &store),
            Outcome::Spawn(super::super::host::Job::FleetStart("glm-53".into(), "Review the diff".into()))
        );
    }

    #[test]
    fn steering_sends_only_while_working() {
        let store = Store::new();
        let mut st = FleetState { ops: vec![op("m", Phase::Starting, None)], steer: "focus tests".into(), ..Default::default() };
        assert_eq!(perform(&mut st, "b3.fleet.steer", 0, &store), Outcome::Done);
        st.ops[0].phase = Phase::Working;
        assert_eq!(perform(&mut st, "b3.fleet.steer", 0, &store), Outcome::Spawn(super::super::host::Job::FleetSteer(0, "focus tests".into())));
    }

    #[test]
    fn model_keys_read_as_versions() {
        assert_eq!(model_label("glm-53"), "glm-5.3");
        assert_eq!(model_label("deepseek-v4-flash"), "deepseek-v4-flash");
    }

    #[test]
    fn the_panel_lowers_balanced() {
        let store = Store::new();
        store.set_active(Some("s".into()));
        let st = FleetState { lanes: vec!["glm-53".into()], ops: vec![op("glm-53", Phase::Working, Some("x"))], ..Default::default() };
        let mut d = Dsl::new();
        build(&mut d, &st, &Frame::DESKTOP, &store);
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.iter().any(|(_, e)| e == "b3.close"));
        assert!(taps.iter().any(|(_, e)| e.starts_with("b3.fleet.steer#")));
    }

    #[test]
    fn the_lane_summary_reads_the_servers_row_and_falls_back_to_the_key() {
        let full = LaneInfo {
            key: "strong".into(),
            provider: "anthropic".into(),
            model: Some("claude-3.5-sonnet".into()),
            description: Some("Strong coding model".into()),
        };
        assert_eq!(full.title(), "anthropic/claude-3.5-sonnet");
        assert_eq!(full.initial(), "A");
        let bare = LaneInfo { key: "glm-53".into(), ..Default::default() };
        assert_eq!(bare.title(), "glm-5.3", "no provider/model: the lane key, version-read");
        assert_eq!(bare.initial(), "G");
    }
}
