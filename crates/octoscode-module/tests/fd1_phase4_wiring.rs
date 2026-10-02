//! #D1 / #A2 — board 1's LIVE surfaces are wired, and each control reaches its
//! owner.
//!
//! The app mounts board 1 as native views (`screens::board1`) in its own dock,
//! opened from real entries: the first-run Connect card's "Pair with a link
//! instead", and the Settings drawer's "Edit provider…", "Open a workspace…"
//! and "Connection…" rows. lib.rs routes every click through
//! `board1::collect` → `perform_board1` → `board1::route`, so this file pins:
//!   1. every control a mounted view publishes is in that view's DSL and is
//!      routed by exactly one owner (a control with no owner is a dead tap —
//!      the #35a/#40b failure class);
//!   2. the entries exist where the app mounts them: the injection lands in
//!      the REAL lowered setup-01 Connect card, and the Settings rows publish
//!      their openers;
//!   3. the routing walk: from each entry, the router reaches every board-1
//!      screen and asks for exactly the work a click must cause (the in-app
//!      click walk — docs/ux/board1/walk-*.log — drives these same calls).
use octoscode_module::screens::{board1, browser, pairing, provider};

/// The host state is process-global (it is the app's live state), so the
/// tests that walk it run one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

const SIZES: [(f64, f64); 2] = [(990.0, 603.0), (412.0, 794.0)];

/// Every board-1 view at both sizes, in each of its states.
fn all_views() -> Vec<(String, board1::Ui)> {
    let mut out = Vec::new();
    for (w, h) in SIZES {
        for (screen, card) in pairing::Screen::ALL {
            let mut p = pairing::PairingUi::new();
            p.screen = screen;
            p.problem = Some(pairing::Problem::Pairing(
                octoscode_client::pairing::PairingErrorKind::CodeUnknown,
            ));
            let l = board1::Layout::of(board1::Surface::Pairing, w, h);
            out.push((format!("{card}@{w}"), pairing::view(&p, &l)));
        }
        for rejected in [false, true] {
            let mut p = provider::ProviderUi::new("deepseek");
            if rejected {
                p.reject("HTTP 401");
            }
            let l = board1::Layout::of(board1::Surface::Provider, w, h);
            out.push((format!("provider rejected={rejected}@{w}"), provider::view(&p, &l)));
        }
        for refused in [false, true] {
            let mut b = browser::BrowserUi {
                path: "/home/user/code".into(),
                entries: ["octos", "octoscode-app", "notes", "scratch"]
                    .iter()
                    .map(|n| browser::Entry { name: (*n).into(), path: format!("/home/user/code/{n}") })
                    .collect(),
                writable: true,
                ..Default::default()
            };
            if refused {
                b.refuse(browser::refusal_from(true, "workspace_list_permission_denied", None).unwrap(), Some("/private".into()));
            }
            let l = board1::Layout::of(board1::Surface::Browser, w, h);
            out.push((format!("browser refused={refused}@{w}"), browser::view(&b, &l)));
        }
    }
    out.push(("settings rows".into(), board1::settings_entries()));
    out
}

fn owner_count(action: &str) -> usize {
    [pairing::is_action(action), provider::is_action(action), browser::is_action(action)]
        .iter()
        .filter(|b| **b)
        .count()
        + board1::OPENERS.iter().filter(|(a, _)| *a == action).count()
        + board1::PICKER_ACTIONS.iter().filter(|(a, _)| *a == action).count()
}

#[test]
fn every_mounted_control_has_exactly_one_owner_that_routes_it() {
    for (what, ui) in all_views() {
        assert!(!ui.buttons.is_empty(), "{what}: no controls at all");
        for (id, action) in ui.buttons.iter().chain(ui.inputs.iter()).chain(ui.returns.iter()) {
            assert_eq!(owner_count(action), 1, "{what}: {id} -> {action} must have ONE owner");
            assert!(board1::owns(action), "{what}: {id} -> {action} is not routed by board 1");
            let routed = pairing::is_routed(action)
                || provider::is_routed(action)
                || browser::is_routed(action)
                || board1::OPENERS.iter().chain(board1::PICKER_ACTIONS).any(|(a, _)| a == action);
            assert!(routed, "{what}: {action} is declared but not routed");
        }
    }
    assert_eq!(pairing::unrouted(), Vec::<&str>::new());
    assert_eq!(provider::unrouted(), Vec::<&str>::new());
    assert_eq!(browser::unrouted(), Vec::<&str>::new());
}

#[test]
fn every_published_control_is_in_its_own_view() {
    // A control published for routing that the DSL lost would be a tap with no
    // widget under it — the instrument would show no rect to click.
    for (what, ui) in all_views() {
        for (id, _) in ui.buttons.iter().chain(ui.inputs.iter()) {
            assert!(ui.dsl.contains(&format!("{id} := ")), "{what}: {id} is published but not in the view");
        }
    }
}

#[test]
fn the_connect_screen_and_the_settings_rows_open_board_one() {
    // The REAL first-run card (setup-01, the production lowering) gains the
    // pairing entry where lib.rs mounts it.
    let ui = octoscode_module::screens::connect::ConnectUi::default();
    let card = octoscode_module::screens::connect::lower_screen(octoscode_module::screens::connect::Screen::Connect, &ui)
        .expect("the Connect card lowers");
    let with = board1::with_connect_entry(&card);
    assert!(with.contains("b1_connect_pair := ButtonFlat"), "the entry is injected into the real card");
    assert!(with.contains("Pair with a link instead"));
    let entries = board1::entry_controls();
    for (id, action) in [
        ("b1_connect_pair", "b1.open.pairing"),
        ("b1_set_provider", "b1.open.provider"),
        ("b1_set_workspace", "b1.open.picker"),
        ("b1_set_connection", "b1.open.connection"),
    ] {
        assert!(entries.iter().any(|(i, a)| i == id && a == action), "{id} -> {action} is not an entry");
    }
}

#[test]
fn the_routing_walk_reaches_every_screen_from_its_entry() {
    let _s = serial();
    board1::close_all();
    let link = |code: &str| format!("http://app.invalid/?octos=http://127.0.0.1:8422&pair={code}");

    // Connect screen -> Pair with Octos (p4-01).
    assert!(board1::route("b1.open.pairing", None).is_empty());
    assert_eq!(board1::top(), Some(board1::Surface::Pairing));
    assert_eq!(pairing::state().screen, pairing::Screen::Pair);
    assert!(board1::view(990.0, 603.0).unwrap().contains("b1_pair_submit := ButtonFlat"));

    // Paste + Pair -> exactly one exchange, p4-02 up.
    board1::route("pair.paste", Some(&link("USED0000")));
    let work = board1::route("pair.submit", None);
    let generation = match work.as_slice() {
        [board1::Work::PairExchange { generation, .. }] => *generation,
        other => panic!("Pair must start one exchange: {other:?}"),
    };
    assert_eq!(pairing::state().screen, pairing::Screen::Pairing);
    // The server says the code was used -> p4-03 with the origin prefilled.
    pairing::finish_exchange(&mut pairing::state(), generation, Err(octoscode_client::pairing::PairingErrorKind::CodeUnknown));
    board1::mark_dirty();
    assert_eq!(pairing::state().screen, pairing::Screen::LinkProblem);
    let v = board1::view(990.0, 603.0).unwrap();
    assert!(v.contains("This pairing link was already used.") && v.contains("http://127.0.0.1:8422"), "p4-03");

    // Back to p4-01; "Enter server and token instead" leaves for the form.
    assert!(board1::route("pair.back", None).is_empty());
    assert_eq!(pairing::state().screen, pairing::Screen::Pair);
    assert_eq!(board1::route("pair.fallback", None), vec![board1::Work::LeaveToForm { server: None }]);
    assert!(!board1::is_open(), "the form is the Connect screen under the dialog");

    // Settings -> Connection (p4-05) -> Forget.
    board1::route("b1.open.connection", None);
    assert_eq!(pairing::state().screen, pairing::Screen::Paired);
    assert!(board1::view(990.0, 603.0).unwrap().contains("Forget this device"));
    assert_eq!(board1::route("pair.forget", None), vec![board1::Work::Forget]);
    assert!(!board1::is_open());

    // Settings -> Edit provider (p4-06): load, then Save is ONE transport.
    assert_eq!(board1::route("b1.open.provider", None), vec![board1::Work::ProviderLoad]);
    assert_eq!(board1::top(), Some(board1::Surface::Provider));
    let before = board1::view(990.0, 603.0).unwrap();
    // Typing never rebuilds the view (the field keeps focus and caret).
    assert!(board1::route("provider.key", Some("sk-typed")).is_empty());
    assert_eq!(board1::view(990.0, 603.0).unwrap(), before, "typing must not rebuild the view");
    assert_eq!(board1::route("provider.save", None), vec![board1::Work::ProviderTransport(provider::Effect::Save)]);
    provider::state().busy = false;
    board1::route("provider.cancel", None);
    assert!(!board1::is_open());

    // Settings -> Open a workspace (the picker) -> Browse folders (p4-08).
    assert_eq!(board1::route("b1.open.picker", None), vec![board1::Work::PickerLoad]);
    // Fail closed: without the advertised feature Browse does nothing.
    board1::note_context(&board1::Context::default());
    assert!(board1::route("picker.browse", None).is_empty());
    assert_eq!(board1::top(), Some(board1::Surface::Picker));
    assert!(!board1::view(990.0, 603.0).unwrap().contains("b1_pk_browse"), "no Browse affordance (row 166)");
    board1::note_context(&board1::Context { capabilities: vec![browser::BROWSE_FEATURE.into()], ..Default::default() });
    board1::mark_dirty();
    assert!(board1::view(990.0, 603.0).unwrap().contains("b1_pk_browse := ButtonFlat"));
    let w = board1::route("picker.browse", None);
    assert!(matches!(w.as_slice(), [board1::Work::BrowserList { resolve_ancestor: true, .. }]), "{w:?}");
    assert_eq!(board1::top(), Some(board1::Surface::Browser));
    // The browser's back returns to the picker; the picker's closes.
    board1::route("browser.close", None);
    assert_eq!(board1::top(), Some(board1::Surface::Picker));
    board1::route("picker.close", None);
    assert!(!board1::is_open());
}

#[test]
fn escape_steps_back_like_each_back_chevron() {
    let _s = serial();
    board1::close_all();
    board1::route("b1.open.picker", None);
    assert_eq!(board1::escape_action(), "picker.close");
    board1::route("b1.open.pairing", None);
    assert_eq!(board1::escape_action(), "pair.back");
    board1::close_all();
}
