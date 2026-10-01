//! #D1 — the nine phase4 cards are wired, and each control reaches its owner.
//!
//! This is the test #35b's own failure mode demands. `taps::wire_card_events`
//! silently wires NOTHING for a card whose `service-actions.json` it cannot
//! read, or whose control bounds miss every lowered button
//! (`taps.rs:44-56`, the 1.5px tolerance; #35d lost `btn_diag` to a 1px drift).
//! setup-08/11 shipped exactly that way and every button was dead until #35b
//! generalised the helper.
//!
//! So for each of the nine cards this test lowers it through the PRODUCTION
//! entry point (`screens::<set>::lower_screen`) and then checks three things:
//!   1. every non-`input.*` control in the card's `service-actions.json` got an
//!      `on_click` — a control with no handler is a dead control;
//!   2. the handler carries that control's OWN event id, not a neighbour's;
//!   3. the event id is claimed by the one set that owns it (`is_action`), so a
//!      tap cannot die as Unhandled in `perform_screen_action`.
//!
//! It also pins the ownership rule itself: the three sets must not overlap, or
//! the last registration would win (the F1 x F3 collision, LESSONS "One owner
//! per notification").

use octoscode_module::screens::{browser, pairing, provider, taps};

/// `(card dir, owning set, the controls that are taps)`.
/// The `input.*` entries are live text, not taps, and are excluded by the
/// helper itself (`taps.rs:107-109`), so they are not listed here.
fn cards() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    vec![
        // p4-01 Pair this device. NOTE: `pair.scan` (the QR viewfinder) is
        // absent on purpose -- the card authors NO button node for it, so there
        // is nothing for the helper to attach a handler to. The tap cannot
        // exist; that is a card gap, not a wiring one, and it is asserted
        // explicitly in `the_viewfinder_tap_is_a_known_card_gap` below.
        ("p4-01", "pairing", vec!["pair.submit", "pair.fallback"]),
        // p4-02 Pairing…
        ("p4-02", "pairing", vec!["pair.cancel", "pair.back"]),
        // p4-03 Link problem -> the manual connect form
        ("p4-03", "pairing", vec!["connect.submit"]),
        // p4-04 Can't pair
        ("p4-04", "pairing", vec!["pair.fallback"]),
        // p4-05 Paired
        ("p4-05", "pairing", vec!["pair.back", "pair.forget"]),
        // p4-06 Provider editor
        (
            "p4-06",
            "provider",
            vec![
                "provider.key.reveal",
                "provider.cancel",
                "provider.save",
                "provider.model.0",
                "provider.model.1",
                "provider.model.2",
            ],
        ),
        // p4-07 Provider rejected
        (
            "p4-07",
            "provider",
            vec![
                "provider.key.reveal",
                "provider.cancel",
                "provider.retry",
                "provider.back",
                "provider.model.0",
                "provider.model.1",
                "provider.model.2",
            ],
        ),
        // p4-08 Choose a folder
        (
            "p4-08",
            "browser",
            vec![
                "browser.enter.0",
                "browser.enter.1",
                "browser.enter.2",
                "browser.enter.3",
                "browser.use",
            ],
        ),
        // p4-09 Folder refused
        ("p4-09", "browser", vec!["browser.back"]),
    ]
}

fn lower(card: &str, set: &str) -> String {
    match (card, set) {
        ("p4-01", "pairing") => pairing::lower_screen(pairing::Screen::Pair, &pairing::PairingUi::new()),
        ("p4-02", "pairing") => pairing::lower_screen(pairing::Screen::Pairing, &pairing::PairingUi::new()),
        ("p4-03", "pairing") => pairing::lower_screen(pairing::Screen::LinkProblem, &pairing::PairingUi::new()),
        ("p4-04", "pairing") => pairing::lower_screen(pairing::Screen::NoPairing, &pairing::PairingUi::new()),
        ("p4-05", "pairing") => pairing::lower_screen(pairing::Screen::Paired, &pairing::PairingUi::new()),
        ("p4-06", "provider") => provider::lower_screen(provider::Screen::Editor, &provider::ProviderUi::new("deepseek")),
        ("p4-07", "provider") => provider::lower_screen(provider::Screen::Rejected, &provider::ProviderUi::new("deepseek")),
        ("p4-08", "browser") => browser::lower_screen(browser::Screen::Browser, &browser::BrowserUi::default()),
        ("p4-09", "browser") => browser::lower_screen(browser::Screen::Refused, &browser::BrowserUi::default()),
        _ => panic!("no lower path for {card}/{set}"),
    }
    .unwrap_or_else(|e| panic!("{card} must lower: {e}"))
}

#[test]
fn every_phase4_control_is_wired_to_its_own_event() {
    for (card, set, expected) in cards() {
        let dsl = lower(card, set);
        let wired = taps::wired_taps(&dsl);
        let got: Vec<String> = wired.iter().map(|(_, ev)| ev.clone()).collect();
        for want in expected {
            assert!(
                got.iter().any(|ev| ev == want),
                "{card}: control {want:?} has no on_click — wired: {got:?}"
            );
        }
        // A card may not wire an event it does not declare: that would route a
        // tap to an id the set does not own.
        for ev in &got {
            let owned = match set {
                "pairing" => pairing::is_action(ev),
                "provider" => provider::is_action(ev),
                "browser" => browser::is_action(ev),
                other => panic!("unknown set {other}"),
            };
            assert!(owned, "{card}: wired {ev:?}, which {set} does not own");
        }
    }
}

#[test]
fn the_tap_count_matches_the_cards_declared_click_controls() {
    // A count mismatch means a control was dropped OR one was injected that the
    // card never asked for — both are wiring bugs the #35a click audit exists
    // to catch.
    for (card, set, expected) in cards() {
        let dsl = lower(card, set);
        let wired = taps::wired_taps(&dsl);
        assert_eq!(
            wired.len(),
            expected.len(),
            "{card}: wired {} tap(s), expected {} — {wired:?}",
            wired.len(),
            expected.len()
        );
    }
}

#[test]
fn the_viewfinder_tap_is_a_known_card_gap() {
    // p4-01 declares `pair.scan` (the QR viewfinder) in service-actions.json,
    // but the card authors NO button node for it — the viewfinder is a plain
    // stack of surface + corner brackets. `taps.rs` can therefore never attach a
    // handler, and the tap is dead by construction, not by a wiring bug.
    //
    // This is recorded, not papered over: the QR intake needs the card to gain a
    // button node (and the shell's scanner API) before `pair.scan` can fire.
    // Until then the atlas's screen 1 is a DESIGN-complete but not yet
    // TAP-complete surface.
    let dir = octoscode_module::design::dir("stage-b/phase4/cards").join("p4-01");
    let sa: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("service-actions.json")).expect("service-actions.json"),
    )
    .expect("parses");
    assert_eq!(sa["controls"]["pair_scan"]["event"], "pair.scan");

    let data: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("page.data.json")).expect("page.data.json"),
    )
    .expect("parses");
    let placements = data["$kit"]["placements"].as_object().expect("placements");
    assert!(
        !placements.contains_key("pair_scan_control"),
        "if the card HAS gained a button node, this gap is fixed — update the \
         test and the p4-01 expected tap list instead of leaving it stale"
    );
}

#[test]
fn a_tap_never_dies_as_unhandled_by_its_owner() {
    // The #40b lesson: an id the audit emits must pass `is_action` or the tap
    // dies as Unhandled before `resolve` ever sees it.
    for (_, set, expected) in cards() {
        for id in expected {
            let routed = match set {
                "pairing" => pairing::is_routed(id),
                "provider" => provider::is_routed(id),
                "browser" => browser::is_routed(id),
                other => panic!("unknown set {other}"),
            };
            assert!(routed, "{set}: {id:?} is declared but not routed");
        }
    }
    // And the coverage contract each set carries.
    assert_eq!(pairing::unrouted(), Vec::<&str>::new());
    assert_eq!(provider::unrouted(), Vec::<&str>::new());
    assert_eq!(browser::unrouted(), Vec::<&str>::new());
}

#[test]
fn the_three_sets_do_not_overlap() {
    // One owner per action id: an id claimed by two sets would be routed by
    // whichever `is_action` the shell consults first (the F1 x F3 collision).
    let sets: [(&str, fn(&str) -> bool); 3] = [
        ("pairing", pairing::is_action),
        ("provider", provider::is_action),
        ("browser", browser::is_action),
    ];
    for (name, owns) in sets {
        for (other, other_owns) in sets {
            if name == other {
                continue;
            }
            for (id, _) in all_actions() {
                if owns(id) {
                    assert!(
                        !other_owns(id),
                        "{id:?} is claimed by both {name} and {other}"
                    );
                }
            }
        }
    }
}

fn all_actions() -> Vec<(&'static str, &'static str)> {
    pairing::ACTIONS
        .iter()
        .chain(provider::ACTIONS.iter())
        .chain(browser::ACTIONS.iter())
        .copied()
        .collect()
}

#[test]
fn every_action_id_the_cards_emit_is_a_known_id() {
    // The cards' OWN service-actions.json is the source of truth for what they
    // emit; every one of those ids must be routed, or the button is dead.
    for (card, set, _) in cards() {
        let dir = octoscode_module::design::dir("stage-b/phase4/cards").join(card);
        let text = std::fs::read_to_string(dir.join("service-actions.json"))
            .unwrap_or_else(|e| panic!("{card}/service-actions.json: {e}"));
        let v: serde_json::Value = serde_json::from_str(&text).expect("service-actions.json parses");
        let controls = v["controls"].as_object().expect("controls object");
        assert!(!controls.is_empty(), "{card} declares no controls");
        for (name, c) in controls {
            let event = c["event"].as_str().unwrap_or_else(|| panic!("{card}/{name}: no event"));
            if event.starts_with("input.") {
                continue; // live text, not a tap
            }
            let routed = match set {
                "pairing" => pairing::is_routed(event),
                "provider" => provider::is_routed(event),
                "browser" => browser::is_routed(event),
                other => panic!("unknown set {other}"),
            };
            assert!(
                routed,
                "{card}: the card emits {event:?}, which {set} does not route — the tap would die"
            );
        }
    }
}
