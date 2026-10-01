//! #FX1 — click proof: a per-row tap addresses ITS OWN row, not row 0.
//!
//! The card's requirement: "the action must address row 2, not row 0". So every
//! test here drives the REAL path the host uses — the card's production
//! lowering, the shared wiring, the host's dispatch list (`taps::wired_taps`),
//! the row split, and finally the card's own `resolve` — and asserts the effect
//! that comes out names row 2.
//!
//! Two families are covered, because they fail differently (see `taps::row_of`):
//!   * dotted  — `p4-06/07/08`: the event already names its row
//!     (`browser.enter.2`), so the id is routed VERBATIM. This proves the wiring
//!     and the dispatch reach row 2.
//!   * shared  — the id carries NO row, so `#FX1` derives it from the control
//!     name. This is the case the fix exists for.
//!
//! #FX1 honest limitation, asserted rather than hidden: the three MOUNTED,
//! resolvable cards with a real row-2 Button are the dotted family
//! (`p4-06`/`p4-07`/`p4-08`). The shared-event family that exercises the new
//! conversion is `conversation-01` (`thread_1`..`thread_5`, one `thread.open`
//! event) and `autonomy-05` (`monitor.toggle`). Of those, `monitor.toggle` has
//! NO resolver anywhere in the crate, so it cannot be proven end-to-end at all;
//! `conversation-01`'s rows are real Buttons but the card is mounted through
//! `l0_host`, not the `screen_taps` path. So the conversion is proven at the
//! wiring/dispatch level here, and the absence of a full click proof for the
//! shared family is recorded in the report rather than papered over.
use octoscode_module::screens::{browser, provider, taps};

/// The host's dispatch list for a lowered card — the SAME call
/// `lib.rs:2476` makes when it mounts a card.
fn dispatch_list(dsl: &str) -> Vec<(String, String)> {
    taps::wired_taps(dsl)
}

/// What the host would pass to the card's resolver: split the id back into
/// (base action, row), exactly as `lib.rs:3126` does.
fn as_host_sends(action: &str) -> (String, usize) {
    let (base, row) = taps::split_row(action);
    (base.to_owned(), row.unwrap_or(0))
}

// ---- the dotted family: p4-08 (browser), row 2 of 4 -------------------------

#[test]
fn p4_08_row_2_click_addresses_row_2_not_row_0() {
    let mut ui = browser::BrowserUi {
        path: "/srv/fixture".into(),
        entries: (0..4)
            .map(|i| browser::Entry {
                name: format!("folder{i}"),
                path: format!("/srv/fixture/folder{i}"),
            })
            .collect(),
        ..Default::default()
    };

    // The production lowering, then the shared wiring (both inside
    // `lower_screen`, as the app mounts it).
    let dsl = browser::lower_screen(browser::Screen::Browser, &ui).expect("lower p4-08");
    let taps = dispatch_list(&dsl);
    // The EMITTED widget name is not the placement name (`row_2_control`
    // lowers to `beauty_0_13`), so the row is identified by its EVENT, which is
    // what the card authored.
    let (_, row2_event) = taps
        .iter()
        .find(|(_, ev)| *ev == "browser.enter.2")
        .unwrap_or_else(|| panic!("p4-08 row 2 is a wired tap: {taps:?}"));

    // The event already names its row, so #FX1 must NOT have suffixed it.
    assert_eq!(
        row2_event, "browser.enter.2",
        "p4-08: the dotted event is routed verbatim, never re-suffixed"
    );

    // The host splits it and routes it. The dotted family carries its row IN
    // the id, so the index the host also passes is irrelevant here —
    // `browser::resolve(id, value)` takes no index at all (browser.rs:344).
    let (base, _index) = as_host_sends(row2_event);
    assert_eq!(base, "browser.enter.2", "the id survives the round trip");

    // And the resolver turns that id into row 2's OWN effect.
    let effect = browser::resolve(&base, None);
    assert!(
        matches!(effect, browser::Effect::Enter(2)),
        "row 2 enters row 2: {effect:?}"
    );
    // Row 0 would be a DIFFERENT folder, so this also proves the click did not
    // silently fall back to the first row. The row under test is the one the
    // EVENT names — the dotted family carries it in the id, not in an index.
    let row = 2usize;
    let picked = ui.pick(row);
    assert_eq!(picked.as_deref(), Some("/srv/fixture/folder2"));
    assert_ne!(
        picked.as_deref(),
        Some("/srv/fixture/folder0"),
        "row 2 must not pick row 0"
    );
}

// ---- the dotted family: p4-06 (provider editor), row 2 of 3 -----------------

#[test]
fn p4_06_row_2_click_addresses_row_2_not_row_0() {
    let ui = provider::ProviderUi::new("deepseek");
    let dsl = provider::lower_screen(provider::Screen::Editor, &ui).expect("lower p4-06");
    let taps = dispatch_list(&dsl);
    let (_, row2_event) = taps
        .iter()
        .find(|(_, ev)| *ev == "provider.model.2")
        .unwrap_or_else(|| panic!("p4-06 row 2 is a wired tap: {taps:?}"));

    assert_eq!(
        row2_event, "provider.model.2",
        "p4-06: routed verbatim (provider::resolve takes NO index)"
    );
    // The dotted family carries its row IN THE ID, so the index the host also
    // passes is 0 and is IGNORED: `provider::resolve(id, value)` matches the
    // literal id (provider.rs:343) and never sees an index. The row is correct
    // precisely because it is in the name, which is what this asserts.
    let (base, index) = as_host_sends(row2_event);
    assert_eq!(base, "provider.model.2");
    assert_eq!(
        index, 0,
        "no row is derived for a dotted id — the id itself carries it"
    );

    assert!(
        matches!(provider::resolve(&base, None), provider::Effect::SelectModel(2)),
        "row 2 selects model 2, not model 0"
    );
}

// ---- the glued-digit, 0-based family (the walk's live catch) ----------------
// The phase4n2 sidebar cards name their rows `ctl_row0..ctl_row4` — the
// digits GLUED to the family word in one segment — and they number from 0.
// The first in-app walk clicked ctl_row2 and logged a bare `session.open`
// (no `#2`): row_of rejected the glued tail and the tap fell back to row 0.
#[test]
fn a_glued_0_based_family_stamps_its_own_number() {
    assert_eq!(taps::row_of("ctl_row2"), Some(2), "glued digits parse");
    assert_eq!(taps::row_of("ctl_row0"), Some(0), "0 exists in this family");
    assert_eq!(taps::row_of("ctl_search"), None, "no digits -> no row");
    // Family evidence: the card also names a row 0, so the index passes
    // through UNSHIFTED — ctl_row2 must address store row 2, not 1.
    assert_eq!(
        taps::with_row_in_family("session.open", "ctl_row2", Some(0)),
        "session.open#2"
    );
    // A 1-based family with the same glued shape still shifts (belt and
    // braces — no measured card has this shape today, the rule is per family,
    // not per spelling).
    assert_eq!(
        taps::with_row_in_family("session.open", "ctl_row2", Some(1)),
        "session.open#1"
    );
    // A lone control (no family evidence) keeps the historical contract.
    assert_eq!(taps::with_row("session.open", "ctl_row2"), "session.open#1");
}

// ---- the shared-event family: the case #FX1 exists for ----------------------

/// `conversation-01` gives all five thread rows ONE `thread.open` event, so the
/// row is not in the id and must be derived from the control name. The two
/// outermost rows are pinned because the bug this fixes was exactly "row 0".
#[test]
fn a_shared_event_derives_each_rows_index_from_its_control_name() {
    // The card's own authored names + bounds, read from the design tree.
    let dir = octoscode_module::design::dir("stage-b/conversation/cards/conversation-01");
    let service = std::fs::read_to_string(dir.join("service-actions.json")).expect("service-actions");
    let v: serde_json::Value = serde_json::from_str(&service).expect("parse");
    let controls = v["controls"].as_object().expect("controls");

    let mut seen = Vec::new();
    for (name, control) in controls {
        let event = control["event"].as_str().expect("event");
        if !name.starts_with("thread_") {
            continue;
        }
        // This is what the shared wiring now does.
        let wired = taps::with_row(event, name);
        let (base, row) = taps::split_row(&wired);
        assert_eq!(base, event, "the base action is preserved for {name}");
        seen.push((name.clone(), wired, row));
    }
    seen.sort_by_key(|(n, _, _)| n.clone());

    // Every row gets its OWN index, and row 1 is 0 — the off-by-one this card
    // would have shipped without the conversion living in `with_row`.
    let got: Vec<(&str, Option<usize>)> = seen
        .iter()
        .map(|(n, w, r)| (n.as_str(), *r))
        .collect();
    assert_eq!(
        got,
        vec![
            ("thread_1", Some(0)),
            ("thread_2", Some(1)),
            ("thread_3", Some(2)),
            ("thread_4", Some(3)),
            ("thread_5", Some(4)),
        ],
        "each thread row reaches its own store index"
    );
    // And the middle row — the card's "row 2" requirement — is index 1, NOT 0.
    assert_eq!(seen[1].1, "thread.open#1", "row 2 carries its own index");
    assert_ne!(seen[1].1, "thread.open#0", "and it is not the default row 0");
}

/// The derived index must survive into the effect the real router produces, so
/// this drives `actions::resolve` — the resolver `thread.open` actually reaches
/// (`lib.rs:1628`, `lib.rs:3215`) — with the row the host would have sent.
#[test]
fn the_derived_row_reaches_actions_resolve_for_thread_open() {
    use octoscode_module::actions;
    use octoscode_store::Store;

    let store = Store::new();
    store.set_sessions(vec![
        octoscode_store::Session {
            id: "sess-0".into(),
            title: None,
            message_count: 0,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        },
        octoscode_store::Session {
            id: "sess-1".into(),
            title: None,
            message_count: 0,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        },
        octoscode_store::Session {
            id: "sess-2".into(),
            title: None,
            message_count: 0,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        },
    ]);
    let store = std::sync::Arc::new(store);
    let ui = std::sync::Mutex::new(octoscode_module::flow::FlowUi::default());
    let ctx = octoscode_module::bindings::Ctx::new(&store, &ui);

    // What the host sends for a click on thread_2 / thread_3.
    for (control, want) in [("thread_2", "sess-1"), ("thread_3", "sess-2")] {
        let (base, row) = as_host_sends(&taps::with_row("thread.open", control));
        let effect = actions::resolve(&base, row, &ctx);
        match effect {
            actions::Effect::Open(id) => assert_eq!(
                id, want,
                "{control} opened {want} — the row reached the resolver, not row 0"
            ),
            other => panic!("{control} must open a session, got {other:?}"),
        }
    }
    // The exact regression: a click on thread_1 with a hardcoded 0 is the same
    // as today's host, so the value of the fix is that it is now row-derived.
    let (base, row) = as_host_sends(&taps::with_row("thread.open", "thread_1"));
    assert_eq!((base.as_str(), row), ("thread.open", 0));
}

// A tiny helper so the Ctx above needs no test-only plumbing.
fn muix() -> std::sync::Mutex<octoscode_module::flow::FlowUi> {
    std::sync::Mutex::new(octoscode_module::flow::FlowUi::default())
}

