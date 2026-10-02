//! #D2b — the settings half's wiring tests, through the PRODUCTION paths.
//!
//! The confirm-gate proof the entry asks for, at three layers:
//! 1. STRUCTURAL: only the modal card (settings-07) declares
//!    `server.stop.confirm` in its service-actions.json — the General page
//!    (settings-06) carries the request row but NO confirm control, so no
//!    lowering of it can ever send the RPC (taps are driven by the cards'
//!    own control tables);
//! 2. STATE: `server.stop.request` raises the pending flag and
//!    `server.stop.cancel` clears it with ZERO shutdown sends (the cancel
//!    walk's contract);
//! 3. PAYLOAD: the RPC-mapped ids lower to exactly the cited wire shapes
//!    (`server/shutdown` params {}; `permission/profile/set` update
//!    {mode, network, approval_policy}).
use std::path::Path;

use octoscode_store::Store;

use octoscode_module::screens::{settings, taps};

fn card_dir(id: &str) -> std::path::PathBuf {
    // the SAME tree the embed walks (design.rs's build.rs table); the manifest
    // path keeps the test independent of the design module's visibility
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/stage-b/settings/cards").join(id)
}

/// (1) The gate is structural: enumerate every control event the seven cards
/// declare and assert `server.stop.confirm` exists ONLY on settings-07.
#[test]
fn the_stop_confirm_lives_only_on_the_modal_card() {
    let mut confirm_cards = Vec::new();
    for (id, _) in settings::CARDS {
        let text = std::fs::read_to_string(card_dir(id).join("service-actions.json"))
            .unwrap_or_else(|e| panic!("read {id}/service-actions.json: {e}"));
        let v: serde_json::Value = serde_json::from_str(&text).expect("parse");
        let controls = v.get("controls").and_then(|c| c.as_object()).expect("controls");
        for (name, c) in controls {
            let event = c.get("event").and_then(|e| e.as_str()).unwrap_or_default();
            assert!(
                settings::owns(event),
                "{id}/{name} declares {event:?} but screens::settings::owns() rejects it"
            );
            if event == "server.stop.confirm" {
                confirm_cards.push(*id);
            }
        }
        // the General row raises the gate; the modal resolves it
        if *id == "settings-06" {
            assert!(
                controls.values().any(|c| c.get("event").and_then(|e| e.as_str())
                    == Some("server.stop.request")),
                "settings-06 must carry the request row"
            );
        }
    }
    assert_eq!(confirm_cards, vec!["settings-07"], "the confirm must live ONLY on the modal card");
}

/// (1b) The PRODUCTION lowering: settings-06's wired taps carry the request
/// row and never the confirm; settings-07's carry the confirm (and the tap
/// publication `start` uses is exactly this `wired_taps` surface).
#[test]
fn the_lowered_general_page_cannot_confirm_the_stop() {
    let dsl06 = settings::lower("settings-06").expect("lower settings-06");
    let taps06 = taps::wired_taps(&dsl06);
    let events06: Vec<&str> = taps06.iter().map(|(_, e)| e.as_str()).collect();
    assert!(
        events06.contains(&"server.stop.request"),
        "the request row must be wired: {events06:?}"
    );
    assert!(
        !events06.contains(&"server.stop.confirm"),
        "the General page must NOT carry a confirm tap: {events06:?}"
    );
    let dsl07 = settings::lower("settings-07").expect("lower settings-07");
    let taps07 = taps::wired_taps(&dsl07);
    let events07: Vec<&str> = taps07.iter().map(|(_, e)| e.as_str()).collect();
    assert!(
        events07.contains(&"server.stop.confirm") && events07.contains(&"server.stop.cancel"),
        "the modal must wire confirm AND cancel: {events07:?}"
    );
}

/// (2) The stateful gate: request raises, cancel clears, and the shutdown
/// counter stays ZERO across the whole request/cancel round (the walk's
/// cancel proof).
#[test]
fn a_cancelled_stop_never_sends_the_shutdown() {
    settings::apply_ui("server.stop.request");
    assert_eq!(settings::query("set.stop_pending"), Some(serde_json::json!(true)));
    settings::apply_ui("server.stop.cancel");
    assert_eq!(settings::query("set.stop_pending"), Some(serde_json::json!(false)));
    assert_eq!(
        settings::query("set.shutdowns"),
        Some(serde_json::json!(0)),
        "a cancelled stop must not send server/shutdown"
    );
}

/// (3) The RPC payloads, value-exact against the citations in the module doc.
/// The confirm is ADVERTISED-GATED like the web's row (App.tsx:3454): mapped
/// only when the production fold carries `server/shutdown` (flow.rs:1188).
#[test]
fn the_rpc_mapped_ids_lower_to_the_cited_wire_shapes() {
    let store = Store::new();
    // A3: the row also needs a live connection (GeneralSettingsContent.tsx:
    // 129-130 `connectionStatus === "connected"`), so the fold arrives live.
    store.set_connection("Live".into(), true);
    // unadvertised: the confirm maps to NOTHING (the web does not render the
    // row; an unadvertised confirm must not send)
    assert_eq!(settings::action_params("server.stop.confirm", &store), None);
    assert!(!settings::can_stop_server(&store));
    // the production fold arrives (set_supported_methods, flow.rs:1188):
    store
        .domains
        .config
        .set_supported_methods(vec!["server/shutdown".to_owned()]);
    assert!(settings::can_stop_server(&store));
    let (method, params) = settings::action_params("server.stop.confirm", &store).expect("mapped");
    assert_eq!(method, "server/shutdown");
    assert_eq!(params, serde_json::json!({}));

    let (method, params) = settings::action_params("perm_ask.select", &store).expect("mapped");
    assert_eq!(method, "permission/profile/set");
    assert_eq!(
        params["update"],
        serde_json::json!({"mode": "workspace_write", "network": "deny", "approval_policy": "ask"})
    );
    let (_, params) = settings::action_params("perm_workspace.select", &store).expect("mapped");
    assert_eq!(
        params["update"],
        serde_json::json!({"mode": "workspace_write", "network": "deny", "approval_policy": "never"})
    );
    let (_, params) = settings::action_params("perm_full.select", &store).expect("mapped");
    assert_eq!(
        params["update"],
        serde_json::json!({"mode": "danger_full_access", "network": "allow", "approval_policy": "never"})
    );

    // the UI-local ids (the web keeps them client-side) map to NOTHING
    for a in ["server.stop.request", "server.stop.cancel", "sandbox_write.toggle",
              "settings.thinking.on", "settings.defaults.open", "session.take_over"] {
        assert!(
            settings::action_params(a, &store).is_none(),
            "{a} must stay UI-local"
        );
    }
}

/// The one-owner rule covers EVERY id the seven cards declare (data-driven).
#[test]
fn every_declared_settings_event_is_owned() {
    for (id, _) in settings::CARDS {
        let text = std::fs::read_to_string(card_dir(id).join("service-actions.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        for (name, c) in v["controls"].as_object().unwrap() {
            let event = c["event"].as_str().unwrap_or_default();
            assert!(
                settings::owns(event),
                "{id}/{name}: {event:?} unowned — a tap would route nowhere"
            );
        }
    }
}

/// The CLICK-walk closure: EVERY declared control of EVERY card is wired into
/// its OWN card's production lowering (the #35b click-audit class: a control
/// absent from `wired_taps` is dead on the mounted screen). Data-driven over
/// the cards' own service-actions.json — a control added to a card without
/// lowering through fails here.
#[test]
fn every_control_wires_into_its_own_cards_lowering() {
    for (id, _) in settings::CARDS {
        let text = std::fs::read_to_string(card_dir(id).join("service-actions.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let declared: Vec<&str> = v["controls"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|c| c["event"].as_str())
            .collect();
        assert!(!declared.is_empty(), "{id} declares no controls");
        let dsl = settings::lower(id).unwrap_or_else(|e| panic!("lower {id}: {e}"));
        let wired_pairs = taps::wired_taps(&dsl);
        let wired: Vec<&str> = wired_pairs.iter().map(|(_, e)| e.as_str()).collect();
        for event in declared {
            assert!(
                wired.contains(&event),
                "{id}: {event:?} declared but NOT wired into the lowering (dead tap): {wired:?}"
            );
        }
    }
}
