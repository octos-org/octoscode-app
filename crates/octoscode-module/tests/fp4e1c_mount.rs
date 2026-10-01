//! P4e1c — the production-path mount tests for the autonomy cards.
//!
//! The card's own requirement: wiring is proven by a CLICK, so these tests
//! drive the same functions `lib.rs` calls when it mounts
//! autonomy-03/04/05 — [`autonomy::lower_screen`] (the production lowering)
//! and [`taps::wire_events_at`] (the tap injection) — and then assert on
//! [`taps::wired_taps`], the dispatch list the host routes real presses from.
//!
//! Every assertion is about REACHABILITY: an action that appears in
//! `wired_taps` can be reached by a user; one that does not cannot, however
//! well the Stage C tables resolve it (RULES 3).
use octoscode_module::screens::{autonomy as au, taps};

fn card_dir(card: &str) -> std::path::PathBuf {
    octoscode_module::design::dir(&format!("stage-b/autonomy/cards/{card}"))
}

fn seeded() -> au::AutonomyState {
    au::AutonomyState {
        goal: Some(serde_json::json!({
            "goal_id": "goal_01",
            "objective": "r1 replay probe",
            "status": "active",
            "token_budget": 100000000u64,
        })),
        goal_generation: 1,
        loops: vec![serde_json::json!({
            "loop_id": "loop_01", "name": "nightly", "status": "active",
        })],
        monitors: vec![serde_json::json!({
            "monitor_id": "monitor_01", "argv": ["./scripts/watch.sh"],
            "status": "active", "interval_seconds": 30,
        })],
    }
}

/// The goal card's three authored controls, at the card's OWN placements.
///
/// `service-actions.json` carries the Stage-A atlas bounds, which disagree with
/// this card's `page.data.json` by more than the 1.5px the shared helper
/// tolerates (measured: Pause 4px, Stop 16px), so the shared path wires
/// nothing. These are the placements the card actually renders.
const GOAL_CONTROLS: &[(&str, &str, f64, f64)] = &[
    ("pause_btn", "goal.pause", 34.0, 559.0),
    ("stop_btn", "goal.stop", 209.0, 559.0),
];

#[test]
fn the_goal_card_wires_pause_and_stop_onto_real_buttons() {
    let st = seeded();
    let dsl = au::lower_screen(au::Screen3::Goal, &st).expect("lower the goal card");
    let controls: Vec<(String, String, f64, f64)> = GOAL_CONTROLS
        .iter()
        .map(|(n, e, x, y)| (n.to_string(), e.to_string(), *x, *y))
        .collect();
    let (wired, report) = taps::wire_events_at(&dsl, &controls);
    for (name, event, ok) in &report {
        assert!(ok, "goal control {name} ({event}) got no handler");
    }
    let routed = taps::wired_taps(&wired);
    for (_, event, _, _) in GOAL_CONTROLS {
        assert!(
            routed.iter().any(|(_, e)| e == event),
            "{event} is reachable from a click; wired={routed:?}"
        );
    }
    // Every wired event must be an action the module actually routes, or the
    // click would land on `Effect::Unhandled` (RULES: production path).
    for (_, event) in &routed {
        let (base, _) = au::split_row(event);
        assert!(au::is_routed(base), "{event} is not in the module's ROUTED table");
    }
}

/// `clear_goal` is authored on a `Text` node, and `inject_click` attaches only
/// to `DesignNativeButton`. The card draws it; the DSL cannot click it. This
/// test records that as the honest, explicit result rather than letting the
/// card's goal.clear row look reachable when it is not.
/// `clear_goal` is authored on a `Text` node (`page.card:112`), and
/// `inject_click` attaches only to `DesignNativeButton`. Measured: it is not a
/// Button, so no handler can be hung on it. Asserted as NOT reachable rather
/// than asserted as drawn — the row `Session goal: clear the goal` keeps its C
/// with this reason.
#[test]
fn clear_goal_is_not_clickable_and_that_is_recorded() {
    let st = seeded();
    let dsl = au::lower_screen(au::Screen3::Goal, &st).expect("lower the goal card");
    let (wired, _report) = taps::wire_events_at(
        &dsl,
        &[("clear_goal".to_string(), "goal.clear".to_string(), 149.0, 654.0)],
    );
    assert!(
        !taps::wired_taps(&wired).iter().any(|(_, e)| e == "goal.clear"),
        "goal.clear is a Text node: it is NOT user-reachable yet"
    );
}

/// The per-row controls are `Svg` icons, so they cannot carry a click without
/// a new hit target — which would be a new surface, forbidden here. This
/// asserts the real state so the board's C rows keep an honest reason.
#[test]
fn the_per_row_controls_are_svgs_and_cannot_be_clicked_yet() {
    let st = seeded();

    // Monitors: NO Button at all — every per-row control lowers to Svg.
    let dsl = au::lower_screen(au::Screen3::Monitors, &st).expect("lower");
    assert!(
        !dsl.contains("DesignNativeButton"),
        "autonomy-05: the row controls lower to Svg, not Button"
    );
    let (wired, report) = taps::wire_events_at(
        &dsl,
        &[("mon_1_pause".to_string(), "monitor.pause".to_string(), 296.0, 231.0)],
    );
    assert!(!report[0].2, "autonomy-05: no Button exists to click");
    assert!(taps::wired_taps(&wired).is_empty(), "autonomy-05: nothing reachable");

    // Loops: the per-row icons are Svg, but `new_loop` IS a real Button. It
    // wires — to `loop.new`, which the module does NOT route, because
    // `loop/create` is unreachable from the module (a new surface). So the card
    // has a clickable control that would land on Unhandled: recorded, not
    // dressed up as a reachable loop action.
    let dsl = au::lower_screen(au::Screen3::Loops, &st).expect("lower");
    let (wired, report) = taps::wire_events_at(
        &dsl,
        &[("new_loop".to_string(), "loop.new".to_string(), 272.0, 84.0)],
    );
    assert!(report[0].2, "autonomy-04: new_loop is a real Button");
    let routed = taps::wired_taps(&wired);
    assert!(
        routed.iter().all(|(_, e)| !au::is_routed(e)),
        "nothing on the loops card reaches the action table: {routed:?}"
    );
    // The per-row loop icon specifically: not clickable.
    let (_, row) = taps::wire_events_at(
        &dsl,
        &[("loop_1_pause".to_string(), "loop.pause".to_string(), 254.0, 188.0)],
    );
    assert!(!row[0].2, "autonomy-04: the per-row icon is an Svg, not a Button");
    // The shared helper's own view, for the record: it wires the goal card's
    // two Buttons only when it can find them at the ATLAS bounds — which it
    // cannot (the drift above). Proving the shared path is the reason the
    // placement path exists.
    let dsl = au::lower_screen(au::Screen3::Goal, &st).expect("lower");
    let shared = taps::wire_card_events_dir(&dsl, &card_dir("autonomy-03"));
    assert!(
        taps::wired_taps(&shared).is_empty(),
        "the authored-atlas path wires nothing — bounds drift past tolerance"
    );
}

/// `split_row` is what lets one widget name address one ROW: the host's tap
/// dispatch passes a single index (`lib.rs` screen_taps loop), so a per-row
/// action would otherwise always address row 0.
#[test]
fn a_row_suffix_addresses_its_row_and_bare_ids_stay_bare() {
    assert_eq!(au::split_row("monitor.pause#2"), ("monitor.pause", Some(2)));
    assert_eq!(au::split_row("goal.pause"), ("goal.pause", None));
    // A name that merely contains '#' is not a row.
    assert_eq!(au::split_row("goal#x"), ("goal#x", None));
    // Both forms reach the one-owner action table.
    assert!(au::is_action("monitor.pause"));
    assert!(au::is_action("monitor.pause#3"));
    assert!(au::is_routed("loop.delete#1"));
    assert!(!au::is_action("monitor.toggle"), "not an authored action");
    assert!(!au::is_action("loop.new"), "loop/create is a new surface");
}
