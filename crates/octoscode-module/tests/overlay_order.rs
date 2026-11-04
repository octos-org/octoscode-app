//! #28 — the module root's Overlay order. A later sibling paints, and takes
//! presses, above an earlier one, so the order of the docks in `script_mod!`
//! IS the layering: the popups (Stop confirm, sidebar menus, command palette)
//! above every full-window surface, and the toasts above everything. Settings
//! stays below the surfaces: it opens surfaces of its own (Capabilities,
//! Models), which must paint over it. The walk `tools/walk/overlay_layers_walk.py`
//! proves the same on the running app; this pins the order without one.

const LIB: &str = include_str!("../src/lib.rs");

/// Where the root's `<id> := View {` declaration sits in the DSL.
fn at(id: &str) -> usize {
    let decl = format!("        {id} := View {{");
    let hits: Vec<_> = LIB.match_indices(&decl).collect();
    assert_eq!(hits.len(), 1, "{id}: one root-level declaration, found {}", hits.len());
    hits[0].0
}

fn assert_below(lower: &[&str], upper: &[&str]) {
    for l in lower {
        for u in upper {
            assert!(at(l) < at(u), "{u} must be declared after {l}, so it paints above it");
        }
    }
}

const SURFACES: [&str; 7] = [
    "screen_dock", "fleet_dock", "dialog_dock", "board1_dock", "board3_dock", "surfaces_dock", "a9_dock",
];
const POPUPS: [&str; 3] = ["stop_dock", "sb_menu_dock", "palette_dock"];

#[test]
fn popups_paint_above_every_surface() {
    assert_below(&SURFACES, &POPUPS);
}

#[test]
fn popups_stack_stop_then_menus_then_palette() {
    assert_below(&["stop_dock"], &["sb_menu_dock", "palette_dock"]);
    assert_below(&["sb_menu_dock"], &["palette_dock"]);
}

#[test]
fn settings_stays_under_the_surfaces_it_opens() {
    assert_below(&["settings_dock"], &SURFACES);
}

#[test]
fn toasts_stay_last() {
    assert_below(&SURFACES, &["toast_dock"]);
    assert_below(&POPUPS, &["toast_dock"]);
    assert_below(&["settings_dock"], &["toast_dock"]);
}

#[test]
fn the_dimmer_and_review_stay_under_every_layer() {
    // The shared dimmer tints the chrome for Settings and the narrow review;
    // the palette brings its own scrim, above the surfaces.
    assert_below(&["review_dock"], &["settings_dock"]);
    assert!(LIB.contains("dimmer := SolidView"));
    assert!(at("review_dock") > LIB.find("dimmer := SolidView").unwrap());
    assert_below(&["review_dock"], &SURFACES);
    assert_below(&["review_dock"], &POPUPS);
}
