//! M2 — the fleet/peers/control-seat screens are MOUNTED (they were
//! production-path but user-unreachable).
//!
//! The finding this card fixes (#P4z): `screens::fleet::lower` had **0 call
//! sites** — the fleet/peers/control-seat cards had their handlers routed
//! (`fleet::is_action` -> `resolve` -> `spawn` -> the production client) but the
//! lowering was never evaluated into the live view tree, so the rows were
//! user-unreachable. Under RULES 3 that is "missing", not "exists".
//!
//! ## What this test actually pins
//!
//! **Not** the lowered card's contents: `f30c_replay.rs:398-412` already proves
//! `fleet::lower("autonomy-06", &ctx)` carries the live strings, and re-asserting
//! it would be duplicate coverage. What changed in M2 is the **call site in
//! `lib.rs`** plus the `chrome_env` opener, so that is what is pinned here:
//!
//! 1. the authored `autonomy-06` card lowers through the production lowering
//!    chain (`lower_card_src` -> `l0::prepare` -> `design::to_makepad_ui`) and
//!    carries the recorded live values;
//! 2. the mount SLOT the shell mounts into is declared in the live view tree
//!    (`fleet_dock` / `fleet_splash`) — the mount cannot happen without it;
//! 3. `chrome_env` gained the `fleet` arm, which is the only opener: a headless
//!    run cannot click a toggle, and there is **no user tap for the fleet card
//!    yet** (recorded as the honest limit, not claimed as a click);
//! 4. the card's taps need no new wiring — the lowered `on_click: || { NAV(t: …) }`
//!    handlers reach the fleet table through the SHARED `taps::owner_of` router,
//!    which is asserted by the existing `taps` unit tests and re-checked here at
//!    the source level (`fleet::is_action` is registered in the router).
//!
//! ## Honesty note (matches .peer/report-M2.md)
//!
//! The card also requires a CLICK walk + `/snap` numbers + one in-app capture
//! from the **mounted app (not an example host)**. That is NOT achievable in
//! this lane: the module's host is `<octosense> --module octoscode`, which does
//! not exist here, and the one host that does exist (`card-host`) is bundle-only
//! (`card-host --help` lists `--bundle <dir>` and no `--module`). So **every M2
//! row stays C** — this test proves the mount is reachable, not that a person
//! can reach it.
use std::sync::{Arc, Mutex};

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::FlowUi;
use octoscode_module::screens::fleet;

/// The store `f30c_replay` pins the cards against (3 peers, one closed).
fn capture_store() -> Arc<octoscode_store::Store> {
    fleet::capture_store()
}

/// 1. The authored card lowers through the production chain and carries the
///    recorded live values (the same assertions f30c pins, kept short here).
#[test]
fn the_authored_fleet_card_lowers_and_carries_the_live_values() {
    let store = capture_store();
    let ui = Arc::new(Mutex::new(FlowUi::default()));
    let ctx = Ctx::new(&store, &ui);

    let dsl = fleet::lower("autonomy-06", &ctx).expect("the fleet card lowers");
    assert!(!dsl.is_empty(), "the lowered card is not empty");
    // The live roster count and the active goal reach the card's copy slots.
    assert!(
        dsl.contains("Fleet · 3 peers"),
        "the live roster count must reach the lowered card"
    );
    assert!(
        dsl.contains("Fix steer queue"),
        "the active goal must reach the lowered card"
    );
}

/// 2. The mount SLOT exists in the live view tree. `fleet::lower` could be
///    called from anywhere and still never render; the slot is what makes the
///    mount land. Asserted at the source level because the view tree is a
///    makepad DSL string (the same way the shell's other slots are pinned).
#[test]
fn the_mount_slot_is_declared_in_the_live_view_tree() {
    let lib = include_str!("../src/lib.rs");
    assert!(
        lib.contains("fleet_splash := Splash"),
        "the fleet Splash slot must be declared in the shell's view tree"
    );
    assert!(
        lib.contains("fleet_dock := View"),
        "the fleet dock View must wrap the Splash slot"
    );
    // …and the shell must actually mount into it.
    assert!(
        lib.contains("ids!(fleet_splash)"),
        "the mount must target the fleet splash slot"
    );
}

/// 3. The opener: `chrome_env` gained the `fleet` arm. This is the ONLY way
///    the card can be shown today (no user tap exists yet).
#[test]
fn chrome_env_grew_the_fleet_opener() {
    let lib = include_str!("../src/lib.rs");
    let body = lib
        .split("fn chrome_env()")
        .nth(1)
        .expect("chrome_env exists");
    let body = &body[..body.find("fn ").unwrap_or(body.len())];
    assert!(
        body.contains("v == \"fleet\""),
        "chrome_env must pre-open the fleet surface for a headless run"
    );
    // The gate the mount reads.
    assert!(
        lib.contains("chrome_env().3"),
        "the fleet mount must gate on the new env arm"
    );
}

/// 4. The taps route without new wiring: the mount publishes the card's wired
///    taps into the shared `screen_taps` slot, and `taps::owner_of` maps them to
///    the fleet table — which is already registered in `perform_screen_action`.
#[test]
fn the_card_taps_route_into_the_fleet_table_without_new_wiring() {
    let lib = include_str!("../src/lib.rs");
    // The fleet table is in the action router (this is what M2 makes reachable).
    assert!(
        lib.contains("screens::fleet::is_action(action)"),
        "the fleet action table must be in the router"
    );
    // The shared tap publisher the mounted card's handlers go through.
    assert!(
        lib.contains("screens::taps::wired_taps"),
        "taps must be published by the shared helper"
    );
    // And the fleet screen still owns its ids.
    let fleet_src = include_str!("../src/screens/fleet.rs");
    assert!(
        fleet_src.contains("pub fn is_action"),
        "the fleet screen must keep owning its action ids (one-owner rule)"
    );
}
