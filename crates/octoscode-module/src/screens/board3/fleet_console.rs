//! A10 — the Fleet's Advanced section: the SESSION CONTROLLER bar the web
//! mounts for the external-driver seats (`SessionControlBar.tsx:786-952`):
//!
//! * the **driver disclosure** — a read-only, keyboard-native
//!   details/summary seat ("External controller" / "Internal controller";
//!   Recovery, and with a binding Driver / Epoch / Revision / Lease), shown
//!   only for a COMPLETE inventory walk (`DriverControllerDisclosure`);
//! * the **control seat** (`PeerControlPanel.tsx`) — mounted only when
//!   control is ready AND this app holds the seat AND a target exists (the
//!   held acquire's pending work + the master's live turn): four commands,
//!   EXACTLY ONE `peer/control` per activation, the bounded refusal label or
//!   the receipt (Worker / Duplicate);
//! * the **peer controller console** (`PeerControllerPanel.tsx`) — re-gated
//!   on `peer/control` + `peer/dispatch`: "Bound to {driver} @ epoch {n}",
//!   its OWN staging (Model lane from the advertised keys only, no default;
//!   Brief; Title), Dispatch (held seat + admitted lane + brief), Release
//!   seat, Acquire seat after a release, and the "Session peers" roster
//!   (glyph + word per activity, Approve / Deny / Steer / Interrupt, a
//!   per-row steer text, the row's receipt or refusal).
use std::collections::HashMap;

use octoscode_store::domains::peer::{Activity, RowControl, RowStatus};
use octoscode_store::Store;

use super::fleetview::FleetState;
use super::host::{Job, Outcome as HostOutcome};
use super::ui::{self, tok, Btn, Dsl, Face, Txt, W};
use crate::screens::fleet_driver;

/// The console's observable state (`PeerControllerPanelState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConsoleOutcome {
    #[default]
    Idle,
    Sending,
    Accepted { slug: String, operation_id: String },
    Refused { dispatch: bool, kind: String },
    Unknown { dispatch: bool },
}

/// The control seat's state (`PeerControlPanelState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SeatPanel {
    #[default]
    Idle,
    Sending(String),
    Receipt { slug: String, duplicate: bool },
    Refused(String),
}

/// The console's own controlled staging + its UI state.
#[derive(Debug, Clone, Default)]
pub struct ConsoleState {
    /// The staged lane KEY ("" = none; never an implicit default).
    pub lane: String,
    pub picker_open: bool,
    pub brief: String,
    pub brief_snap: String,
    pub title: String,
    pub title_snap: String,
    pub outcome: ConsoleOutcome,
    /// Per-row steer text (by roster identity).
    pub steer: HashMap<String, String>,
    pub steer_snap: HashMap<String, String>,
    pub disclosure_open: bool,
    pub seat: SeatPanel,
    /// The roster identities the last lowering drew (`row#n` → n / 4).
    pub drawn: Vec<String>,
    /// The master's live turn (the seat target's expected turn).
    pub live_turn: Option<String>,
}

impl ConsoleState {
    pub fn snap_inputs(&mut self) {
        self.brief_snap = self.brief.clone();
        self.title_snap = self.title.clone();
        self.steer_snap = self.steer.clone();
    }
}

/// The four console row affordances, in product order.
pub const ROW_ACTIONS: [&str; 4] = ["approve", "deny", "steer", "interrupt"];
/// The seat's four commands, in product order.
pub const SEAT_COMMANDS: [(&str, &str); 4] = [
    ("approval_respond", "Respond to approval"),
    ("question_respond", "Answer question"),
    ("steer", "Steer"),
    ("interrupt", "Interrupt"),
];

/// `peerControllerRosterRows` (`peer-controller-staging.ts:348-374`): rows
/// with BOTH an accepted operation id and a turn, opening/started; a closed
/// row stays as `reaped` without affordances.
pub fn console_rows(store: &Store) -> Vec<(String, String, &'static str)> {
    store
        .domains
        .peer
        .rows()
        .into_iter()
        .filter_map(|r| {
            let activity = if r.status == RowStatus::Closed {
                "reaped"
            } else {
                match r.activity {
                    Activity::Blocked => "blocked",
                    Activity::Live => "live",
                    Activity::Done => "done",
                    Activity::Idle => "staged",
                }
            };
            if activity == "reaped" {
                return Some((r.identity, r.slug, activity));
            }
            let has_op = r.operation_id.as_deref().is_some_and(|o| !o.is_empty());
            (has_op && !r.turn_id.is_empty() && matches!(r.status, RowStatus::Opening | RowStatus::Started))
                .then_some((r.identity, r.slug, activity))
        })
        .collect()
}

fn glyph(activity: &str) -> (&'static str, &'static str) {
    match activity {
        "live" => ("✻", "streaming"),
        "blocked" => ("⚠", "needs you"),
        "done" => ("✓", "done"),
        "reaped" => ("✕", "reaped"),
        _ => ("○", "staged"),
    }
}

/// `leaseCopy` (`SessionControlBar.tsx:770-777`): zero is the only "no
/// active lease"; a positive lease prints its ISO instant.
pub fn lease_copy(ms: u64) -> String {
    if ms == 0 {
        return "No active lease".into();
    }
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let date = ui::short_date(ms);
    let _ = days;
    format!(
        "Lease expires {} {:02}:{:02}:{:02} UTC",
        date,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

// ------------------------------------------------------------------ actions

pub fn perform(st: &mut FleetState, rest: &str, index: usize, store: &Store) -> HostOutcome {
    let session = store.active_session().unwrap_or_default();
    let c = &mut st.console;
    match rest {
        "disclosure" => {
            c.disclosure_open = !c.disclosure_open;
            HostOutcome::Done
        }
        "lane.toggle" => {
            if !st.lanes.is_empty() {
                st.console.picker_open = !st.console.picker_open;
            }
            HostOutcome::Done
        }
        "lane" => {
            if let Some(k) = st.lane_keys().get(index).cloned() {
                st.console.lane = k;
                st.console.picker_open = false;
            }
            HostOutcome::Done
        }
        "dispatch" => {
            // `peerControllerStagingSubmit`: the ONE gate — held seat, an
            // ADMITTED lane, a non-blank brief; anything else sends nothing.
            let keys = st.lane_keys();
            let lane = st.console.lane.clone();
            if !fleet_driver::seat_held(&session) || !keys.contains(&lane) || st.console.brief.trim().is_empty() {
                return HostOutcome::Done;
            }
            if st.console.outcome == ConsoleOutcome::Sending {
                return HostOutcome::Done;
            }
            st.console.outcome = ConsoleOutcome::Sending;
            let title = st.console.title.trim().to_owned();
            HostOutcome::Spawn(Job::FleetConsoleDispatch {
                lane,
                brief: st.console.brief.clone(),
                title: (!title.is_empty()).then_some(title),
            })
        }
        "release" => {
            if !fleet_driver::seat_held(&session) {
                return HostOutcome::Done;
            }
            HostOutcome::Spawn(Job::FleetSeatRelease)
        }
        "acquire" => {
            if !fleet_driver::seat_parked(&session) {
                return HostOutcome::Done;
            }
            HostOutcome::Spawn(Job::FleetSeatAcquire)
        }
        "seat" => {
            let Some((kind, _)) = SEAT_COMMANDS.get(index) else { return HostOutcome::Done };
            if matches!(st.console.seat, SeatPanel::Sending(_)) {
                return HostOutcome::Done;
            }
            st.console.seat = SeatPanel::Sending((*kind).to_owned());
            HostOutcome::Spawn(Job::FleetSeatControl { kind: (*kind).to_owned(), live_turn: st.console.live_turn.clone() })
        }
        "row" => {
            let (r, a) = (index / 4, index % 4);
            let Some(identity) = st.console.drawn.get(r).cloned() else { return HostOutcome::Done };
            let action = ROW_ACTIONS[a];
            let text = st.console.steer.get(&identity).cloned().unwrap_or_default();
            if action == "steer" && text.trim().is_empty() {
                return HostOutcome::Done; // a blank steer fails closed here
            }
            HostOutcome::Spawn(Job::FleetConsoleRow { identity, action: action.to_owned(), text })
        }
        _ => HostOutcome::Unrouted,
    }
}

pub fn input_changed(st: &mut FleetState, key: &str, text: &str) {
    match key {
        "brief" => st.console.brief = text.to_owned(),
        "title" => st.console.title = text.to_owned(),
        k => {
            if let Some(i) = k.strip_prefix("steer#").and_then(|n| n.parse::<usize>().ok()) {
                if let Some(id) = st.console.drawn.get(i).cloned() {
                    st.console.steer.insert(id, text.to_owned());
                }
            }
        }
    }
}

// ------------------------------------------------------------- transport

pub async fn run_dispatch(conv: &crate::flow::Conversation, lane: String, brief: String, title: Option<String>) -> Result<String, String> {
    let keys = super::host::state().fleet.lane_keys();
    let session = conv.session_id();
    if !fleet_driver::seat_held(&session) {
        super::host::state().fleet.console.outcome = ConsoleOutcome::Refused { dispatch: true, kind: "driver_fence_stale".into() };
        return Err("no seat".into());
    }
    let mut staged = None;
    let op = octoscode_client::domains::external_driver::new_operation_id();
    let outcome = fleet_driver::start_with(conv, &op, &lane, &keys, &brief, title.as_deref(), false, &mut staged).await;
    let mut st = super::host::state();
    st.fleet.console.outcome = match &outcome {
        fleet_driver::StartOutcome::Accepted { slug, operation_id, .. } => {
            st.fleet.console.brief.clear();
            st.fleet.console.title.clear();
            ConsoleOutcome::Accepted { slug: slug.clone(), operation_id: operation_id.clone() }
        }
        fleet_driver::StartOutcome::Refused { kind } => ConsoleOutcome::Refused { dispatch: true, kind: kind.clone() },
        fleet_driver::StartOutcome::Unknown => ConsoleOutcome::Unknown { dispatch: true },
    };
    st.fleet.console.snap_inputs();
    Ok(format!("{outcome:?}"))
}

pub async fn run_row(conv: &crate::flow::Conversation, identity: String, action: String, text: String) -> Result<String, String> {
    let r = fleet_driver::console_row_control(conv, &identity, &action, &text).await;
    if r.is_ok() && action == "steer" {
        let mut st = super::host::state();
        st.fleet.console.steer.remove(&identity);
        st.fleet.console.steer_snap.remove(&identity);
    }
    r
}

pub async fn run_seat(conv: &crate::flow::Conversation, kind: String, live_turn: Option<String>) -> Result<String, String> {
    let r = fleet_driver::seat_control(conv, &kind, live_turn.as_deref()).await;
    let mut st = super::host::state();
    st.fleet.console.seat = match &r {
        Ok((slug, duplicate)) => SeatPanel::Receipt { slug: slug.clone(), duplicate: *duplicate },
        Err(label) => SeatPanel::Refused(label.clone()),
    };
    r.map(|(s, d)| format!("{s} duplicate={d}"))
}

// -------------------------------------------------------------------- view

fn fact(d: &mut Dsl, id: &str, label: &str, value: &str) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.text(&format!("{id}_k"), label, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Px(84.0)));
    d.text(&format!("{id}_v"), value, &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill).wrap());
    d.close();
}

fn disclosure_seat(d: &mut Dsl, st: &FleetState, store: &Store) {
    let Some(disc) = fleet_driver::disclosure(store) else { return };
    let mode = if disc.mode == "external" { "External controller" } else { "Internal controller" };
    ui::card_open(d, "b3_fleet_disclosure", 6.0);
    d.view("b3_fleet_disclosure_head", "width: Fill height: 30 flow: Overlay");
    let h = d.anon();
    d.view(&h, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} spacing: 6");
    d.icon("", if st.console.disclosure_open { "b3_chevron_down_dark.svg" } else { "b3_chevron_right_dark.svg" }, 12.0, tok::TEXT);
    d.text("b3_fleet_disclosure_mode", mode, &Txt::new(13.0, Face::Medium, tok::TEXT));
    d.close();
    d.tap("b3_fleet_disclosure_toggle", "b3.fleet.console.disclosure");
    d.close();
    if st.console.disclosure_open {
        let recovery = match disc.recovery.as_str() {
            "interrupted" => "Interrupted",
            "recovery_required" => "Recovery required",
            _ => "No recovery pending",
        };
        fact(d, "b3_fleet_disc_recovery", "Recovery", recovery);
        // `peerControlBindingFor`: the HELD acquire's own binding wins over
        // the last observed walk.
        let session = store.active_session().unwrap_or_default();
        let binding = fleet_driver::held_binding(&session)
            .map(|b| (b.driver_id, b.epoch, b.revision, b.lease_expires_at_ms))
            .or(disc.binding.clone());
        if let Some((driver, epoch, revision, lease)) = &binding {
            fact(d, "b3_fleet_disc_driver", "Driver", driver);
            fact(d, "b3_fleet_disc_epoch", "Epoch", &epoch.to_string());
            fact(d, "b3_fleet_disc_revision", "Revision", &revision.to_string());
            fact(d, "b3_fleet_disc_lease", "Lease", &lease_copy(*lease));
        }
    }
    d.close();
}

fn seat_panel(d: &mut Dsl, st: &FleetState, store: &Store) {
    let session = store.active_session().unwrap_or_default();
    if !fleet_driver::seat_held(&session)
        || fleet_driver::seat_target(&session, st.console.live_turn.as_deref()).is_none()
    {
        return;
    }
    ui::card_open(d, "b3_fleet_seat", 8.0);
    ui::section_title(d, "b3_fleet_seat_title", "Peer control");
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 8");
    let sending = matches!(st.console.seat, SeatPanel::Sending(_));
    for (i, (_, label)) in SEAT_COMMANDS.iter().enumerate() {
        d.button(
            &format!("b3_fleet_seat_cmd_{i}"),
            label,
            &format!("b3.fleet.console.seat#{i}"),
            if sending { Btn::OutlineOff } else { Btn::Outline },
            W::Fit,
            32.0,
        );
    }
    d.close();
    match &st.console.seat {
        SeatPanel::Sending(k) => d.text("b3_fleet_seat_state", &format!("Sending {k}…"), &ui::meta().w(W::Fill)),
        SeatPanel::Refused(label) => {
            d.text("b3_fleet_seat_state", label, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap())
        }
        SeatPanel::Receipt { slug, duplicate } => {
            fact(d, "b3_fleet_seat_worker", "Worker", slug);
            fact(d, "b3_fleet_seat_dup", "Duplicate", if *duplicate { "Already applied" } else { "Newly applied" });
        }
        SeatPanel::Idle => {}
    }
    d.close();
}

fn controller_console(d: &mut Dsl, st: &mut FleetState, store: &Store, inner_w: f64) {
    if !fleet_driver::control_advertised(store) {
        return;
    }
    let session = store.active_session().unwrap_or_default();
    let held = fleet_driver::seat_held(&session);
    let parked = fleet_driver::seat_parked(&session);
    ui::card_open(d, "b3_fleet_console", 8.0);
    ui::section_title(d, "b3_fleet_console_title", "Peer controller");
    let binding = fleet_driver::held_binding(&session)
        .map(|b| (b.driver_id, b.epoch))
        .or_else(|| fleet_driver::disclosure(store).and_then(|d| d.binding.map(|b| (b.0, b.1))));
    if let Some((driver, epoch)) = binding {
        d.text(
            "b3_fleet_console_bound",
            &format!("Bound to {driver} @ epoch {epoch}"),
            &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
        );
    }
    // The lane picker: the advertised keys verbatim, no default.
    ui::field_label(d, "b3_fleet_console_lane_label", "Model lane");
    let keys = st.lane_keys();
    let lane = if keys.contains(&st.console.lane) { st.console.lane.clone() } else { String::new() };
    d.surface("b3_fleet_console_lane", "width: Fill height: 34 flow: Overlay", if keys.is_empty() { tok::SURFACE2 } else { tok::SURFACE }, 8.0, Some("#d9d9dcff"));
    let mrow = d.anon();
    d.view(&mrow, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 10 right: 10 top: 0 bottom: 0}");
    d.text("b3_fleet_console_lane_value", &lane, &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
    d.icon("", "chevron_down.svg", 14.0, tok::MUTED);
    d.close();
    if !keys.is_empty() {
        d.tap("b3_fleet_console_lane_tap", "b3.fleet.console.lane.toggle");
    }
    d.close();
    if st.console.picker_open {
        d.surface("b3_fleet_console_opts", "width: Fill height: Fit flow: Down padding: Inset{left: 4 right: 4 top: 4 bottom: 4}", tok::SURFACE, 8.0, Some(tok::HAIRLINE));
        for (i, k) in keys.iter().enumerate() {
            let id = format!("b3_fleet_console_opt_{i}");
            d.surface(&format!("{id}_box"), "width: Fill height: 30 flow: Overlay", if *k == lane { tok::CHIP } else { tok::TRANSPARENT }, 6.0, None);
            let r = d.anon();
            d.view(&r, "width: Fill height: Fill flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{left: 8 right: 8 top: 0 bottom: 0}");
            d.text(&format!("{id}_label"), k, &Txt::new(12.5, Face::Mono, tok::TEXT).w(W::Fill));
            d.close();
            d.tap(&id, &format!("b3.fleet.console.lane#{i}"));
            d.close();
        }
        d.close();
    }
    ui::field_label(d, "b3_fleet_console_brief_label", "Brief");
    d.input("b3_fleet_console_brief", "fleet.console.brief", &st.console.brief_snap, "", false, 34.0);
    ui::field_label(d, "b3_fleet_console_title_label", "Title");
    d.input("b3_fleet_console_title", "fleet.console.title", &st.console.title_snap, "", false, 34.0);
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 8");
    let dispatchable = held && keys.contains(&lane) && !st.console.brief.trim().is_empty();
    let sending = st.console.outcome == ConsoleOutcome::Sending;
    d.button(
        "b3_fleet_console_dispatch",
        "Dispatch",
        "b3.fleet.console.dispatch",
        if dispatchable && !sending { Btn::Primary } else { Btn::Disabled },
        W::Fit,
        32.0,
    );
    d.button("b3_fleet_console_release", "Release seat", "b3.fleet.console.release", if held { Btn::Outline } else { Btn::OutlineOff }, W::Fit, 32.0);
    if parked {
        d.button("b3_fleet_console_acquire", "Acquire seat", "b3.fleet.console.acquire", Btn::Outline, W::Fit, 32.0);
    }
    d.close();
    // "Session peers" — the console roster.
    let rows = console_rows(store);
    st.console.drawn = rows.iter().map(|(id, _, _)| id.clone()).collect();
    if !rows.is_empty() {
        d.text("b3_fleet_console_roster", "Session peers", &Txt::new(12.0, Face::Semibold, tok::MUTED).w(W::Fill));
    }
    for (ri, (identity, slug, activity)) in rows.iter().enumerate() {
        let id = format!("b3_fleet_console_row_{ri}");
        let (g, word) = glyph(activity);
        d.surface(&id, "width: Fill height: Fit flow: Down spacing: 6 padding: Inset{left: 10 right: 10 top: 8 bottom: 8}", tok::SURFACE2, 8.0, None);
        let head = d.anon();
        d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
        d.text(&format!("{id}_glyph"), g, &Txt::new(13.0, Face::Regular, tok::TEXT));
        d.text(&format!("{id}_word"), word, &Txt::new(11.5, Face::Regular, tok::MUTED));
        d.text(&format!("{id}_slug"), &super::inventory::fit(slug, inner_w - 160.0, 12.0, true), &Txt::new(12.0, Face::Mono, tok::TEXT).w(W::Fill));
        d.close();
        if *activity != "reaped" {
            let steer = st.console.steer.get(identity).cloned().unwrap_or_default();
            let acts = d.anon();
            d.view(&acts, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 6");
            for (ai, a) in ROW_ACTIONS.iter().enumerate() {
                let blank = *a == "steer" && steer.trim().is_empty();
                let label = match *a {
                    "approve" => "Approve",
                    "deny" => "Deny",
                    "steer" => "Steer",
                    _ => "Interrupt",
                };
                d.button(
                    &format!("{id}_act_{ai}"),
                    label,
                    &format!("b3.fleet.console.row#{}", ri * 4 + ai),
                    if blank || sending { Btn::OutlineOff } else { Btn::Outline },
                    W::Fit,
                    30.0,
                );
            }
            d.close();
            let snap = st.console.steer_snap.get(identity).cloned().unwrap_or_default();
            d.input(&format!("{id}_steer"), &format!("fleet.console.steer#{ri}"), &snap, "", false, 32.0);
        }
        if let Some(row) = store.domains.peer.row(identity) {
            match row.control {
                Some(RowControl::Receipt { duplicate }) => d.text(
                    &format!("{id}_receipt"),
                    if duplicate { "Already applied" } else { "Newly applied" },
                    &Txt::new(12.0, Face::Medium, tok::GREEN),
                ),
                Some(RowControl::Refused { kind }) => d.text(
                    &format!("{id}_refusal"),
                    fleet_driver::control_refusal_label(&kind),
                    &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap(),
                ),
                _ => {}
            }
        }
        d.close();
    }
    let copy = match &st.console.outcome {
        ConsoleOutcome::Refused { dispatch: true, kind } => Some((fleet_driver::dispatch_refusal_label(kind).to_owned(), tok::RED)),
        ConsoleOutcome::Refused { dispatch: false, kind } => Some((fleet_driver::control_refusal_label(kind).to_owned(), tok::RED)),
        ConsoleOutcome::Unknown { dispatch: true } => Some(("The dispatch could not be confirmed.".to_owned(), tok::RED)),
        ConsoleOutcome::Unknown { dispatch: false } => Some(("The control command could not be confirmed.".to_owned(), tok::RED)),
        ConsoleOutcome::Accepted { .. } => Some(("The peer dispatch was accepted.".to_owned(), tok::GREEN)),
        ConsoleOutcome::Sending => Some(("Working…".to_owned(), tok::MUTED)),
        ConsoleOutcome::Idle => None,
    };
    if let Some((text, color)) = copy {
        d.text("b3_fleet_console_state", &text, &Txt::new(12.0, Face::Regular, color).w(W::Fill).wrap());
    }
    if let ConsoleOutcome::Accepted { slug, operation_id } = &st.console.outcome {
        fact(d, "b3_fleet_console_worker", "Worker", slug);
        fact(d, "b3_fleet_console_operation", "Operation", operation_id);
    }
    d.close();
}

/// The Advanced body: the three seats, in `SessionControlBar` order.
pub fn build(d: &mut Dsl, st: &mut FleetState, store: &Store, inner_w: f64) {
    disclosure_seat(d, st, store);
    seat_panel(d, st, store);
    controller_console(d, st, store, inner_w);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lease_reads_no_active_lease_only_at_zero() {
        assert_eq!(lease_copy(0), "No active lease");
        assert!(lease_copy(1_770_000_000_000).starts_with("Lease expires "));
    }
}
