//! D3b — board-3 screens 5-8: inspector (05), thinking effort (06), resume
//! candidates (07), session strip (08). The one-owner pattern of
//! `screens/pairing` (the #D1 cards): [`card_for`] names the card,
//! [`lower`] runs the SAME chain the Gate-B renders ran — the AUTHORED
//! contract tree (compile.py:49 consumes mapped.json over the contract; v2-v4
//! rendered the measure stage's atlas re-projection, which is why they
//! overlapped — the drop-mapped rule is baked in upstream of this module) —
//! and the ONE shared tap helper wires every CLICK control from the cards'
//! `service-actions.json`. [`owns`]/[`perform`] give the events their store
//! meaning: the thinking prefs, the folded-thinking rows, the resume
//! candidate gate, the strip's read-only projections.
use octoscode_store::Store;

// A4 — the NATIVE board-3 surfaces (all twelve screens). The static Stage-B
// cards above stay for the `OCTOSCODE_SCREEN=p4n3-0N` dev mount; what the
// user reaches is the flow-laid-out dialog family in `board3/` (see
// `board3/ui.rs` for why: runtime lists + a desktop window shorter than the
// 406x776 artboard).
pub mod agents;
pub mod checkpoints;
// A10: the Fleet's Advanced session controller and its zh catalog.
pub mod fleet_console;
pub mod fleet_copy;
pub mod fleetview;
pub mod host;
pub mod images;
pub mod inspector;
pub mod inventory;
pub mod resume;
pub mod rows;
pub mod strip;
pub mod switcher;
pub mod thinking;
pub mod ui;
pub mod vim;

const CARDS: &str = "stage-b/phase4-new3/cards";

/// `OCTOSCODE_SCREEN` → the board-3 card to mount. Only these four; #D1's
/// `p4-*` and the setup cards belong to their own sets.
pub fn card_for(which: &str) -> Option<String> {
    match which {
        "p4n3-05" | "p4n3-06" | "p4n3-07" | "p4n3-08" => Some(which.to_owned()),
        _ => None,
    }
}

/// Lower one board-3 card to Splash DSL with its taps wired — the pairing
/// chain (`pairing.rs:578-581`): prepare the authored card, make it
/// inspectable, to_makepad_ui through the crate's fonts, rename the
/// `beauty_0` collision per screen, then `taps::wire_card_events_dir`.
pub fn lower(screen_id: &str) -> Result<String, String> {
    let dir = crate::design::dir(&format!("{CARDS}/{screen_id}"));
    let read = |rel: &str| -> Result<String, String> {
        std::fs::read_to_string(dir.join(rel))
            .map_err(|e| format!("read {screen_id}/{rel}: {e}"))
    };
    let card_src = read("page.card")?;
    let data: serde_json::Value = serde_json::from_str(&read("page.data.json")?)
        .map_err(|e| format!("parse {screen_id} data: {e}"))?;
    let kit = dir.join("kit");
    let prepared = octoscript_makepad::l0::prepare(&card_src, &data, &kit)
        .map_err(|e| format!("prepare {screen_id}: {e}"))?;
    let mut tree = prepared.tree;
    octoscript_makepad::l0::inspectable(&mut tree);
    let dsl = crate::design::with_fonts(octoscript_makepad::design::to_makepad_ui(&tree))
        .map_err(|e| format!("to_makepad_ui {screen_id}: {e}"))?;
    let prefix = format!("b3_{}", screen_id.trim_start_matches("phase4n3-"));
    let dsl = dsl.replace("beauty_0", &prefix);
    Ok(super::taps::wire_card_events_dir(&dsl, &dir))
}

/// The action ids this set owns — exactly the four cards' service-actions
/// events, one owner per id (the #D1/f29d rule; the wiring test pins it).
pub fn owns(action: &str) -> bool {
    matches!(
        action,
        "inspector_refresh"
            | "inspector_copy_link"
            | "thinking.effort.low"
            | "thinking.effort.medium"
            | "thinking.effort.high"
            | "thinking.effort.max"
            | "thinking.show_reasoning"
            | "thinking.default_new"
            | "thinking.row_0"
            | "thinking.row_1"
            | "thinking_expand_all"
            | "thinking_collapse_all"
            | "resume.select.0"
            | "resume.select.1"
            | "resume.select.2"
            | "resume_confirm"
            | "strip_model"
            | "strip_activity"
            | "strip_permissions"
    )
}

/// The store meaning of each event (sync — these are store writes; the
/// transport-backed reads live in the session domain, already folded).
/// The active session keys the per-session seams; with none active the
/// dsflash fixture id keeps the store usable headless.
pub fn perform(action: &str, store: &Store) -> Result<String, String> {
    let sid = store
        .domains
        .session
        .active()
        .unwrap_or_else(|| "dsflash:main".to_owned());
    match action {
        "thinking.effort.low" | "thinking.effort.medium" | "thinking.effort.high"
        | "thinking.effort.max" => {
            let effort = action.rsplit('.').next().unwrap_or("high");
            store.domains.session.set_thinking_effort(&sid, effort);
            Ok(format!("effort={effort}"))
        }
        "thinking.show_reasoning" => {
            let next = !store.domains.session.thinking(&sid).show_reasoning;
            store.domains.session.set_show_reasoning(&sid, next);
            Ok(format!("show_reasoning={next}"))
        }
        "thinking.default_new" => {
            let next = !store.domains.session.thinking(&sid).default_on;
            store.domains.session.set_thinking_default_on(&sid, next);
            Ok(format!("default_on={next}"))
        }
        "thinking.row_0" | "thinking.row_1" => {
            let row = action.trim_start_matches("thinking.").to_owned();
            let mut prefs = store.domains.session.thinking(&sid);
            if prefs.expanded.iter().any(|e| e == &row) {
                prefs.expanded.retain(|e| e != &row);
            } else {
                prefs.expanded.push(row.clone());
            }
            store
                .domains
                .session
                .set_thinking_expanded(&sid, prefs.expanded);
            Ok(format!("toggled {row}"))
        }
        "thinking_expand_all" => {
            store
                .domains
                .session
                .set_thinking_expanded(&sid, vec!["row_0".into(), "row_1".into()]);
            Ok("expanded all".into())
        }
        "thinking_collapse_all" => {
            store.domains.session.set_thinking_expanded(&sid, vec![]);
            Ok("collapsed all".into())
        }
        "resume.select.0" | "resume.select.1" | "resume.select.2" => {
            let row: usize = action
                .rsplit('.')
                .next()
                .unwrap_or("")
                .parse()
                .map_err(|_| format!("bad candidate row in {action:?}"))?;
            store.domains.session.set_pending_resume(&sid, row);
            Ok(format!("candidate {row} armed — confirm requires the exact typed title"))
        }
        // Fail-closed (the web's disabled-until-confirmed button,
        // candidate-session.ts:230-243): no confirmed source Session without
        // the exact typed match — which the dialog surface (design flow)
        // owns. The gate refuses honestly rather than resuming blind.
        "resume_confirm" => match store.domains.session.pending_resume(&sid) {
            Some(row) => Err(format!(
                "resume row {row} awaits the exact-typed match (the dialog surface owns the input)"
            )),
            None => Err("no candidate selected".into()),
        },
        "inspector_refresh" => Ok("thread graph re-read from the folded session store".into()),
        "inspector_copy_link" => Ok("session link copied (the UI layer owns the clipboard write)".into()),
        "strip_model" | "strip_activity" | "strip_permissions" => {
            Ok("strip segments are read-only projections".into())
        }
        _ => Err(format!("board3: unowned action {action:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_prefs_default_fail_closed_and_flip() {
        let store = Store::new();
        store.domains.session.set_active(Some("dsflash:main".into()));
        // Row 11's contract: show-thinking defaults ON (fails closed). A4:
        // the effort defaults to the Profile default ("") and every block is
        // folded, the web's own defaults (reasoning/model.ts, App.tsx:515).
        let p = store.domains.session.thinking("dsflash:main");
        assert_eq!(p.effort, "");
        assert!(p.show_reasoning);
        assert!(p.default_on);
        assert!(p.expanded.is_empty());
        perform("thinking.effort.low", &store).expect("effort");
        perform("thinking_collapse_all", &store).expect("collapse");
        let p = store.domains.session.thinking("dsflash:main");
        assert_eq!(p.effort, "low");
        assert!(p.expanded.is_empty());
    }

    #[test]
    fn show_reasoning_toggles_per_session() {
        let store = Store::new();
        store.domains.session.set_active(Some("dsflash:main".into()));
        perform("thinking.show_reasoning", &store).expect("toggle");
        assert!(!store.domains.session.thinking("dsflash:main").show_reasoning);
        perform("thinking.show_reasoning", &store).expect("toggle back");
        assert!(store.domains.session.thinking("dsflash:main").show_reasoning);
    }

    /// The #D1/f29d ownership rule: every CLICK control the four cards
    /// declare resolves in [`owns`], and no id is claimed twice across the
    /// seams that share lib.rs's router.
    #[test]
    fn every_card_control_is_owned_and_nobody_else_claims_them() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/phase4-new3/cards");
        for n in ["05", "06", "07", "08"] {
            let text = std::fs::read_to_string(
                root.join(format!("phase4n3-{n}/service-actions.json")),
            )
            .expect("service-actions.json");
            let v: serde_json::Value = serde_json::from_str(&text).expect("json");
            for (name, c) in v["controls"].as_object().expect("controls") {
                let event = c["event"].as_str().expect("event");
                assert!(owns(event), "phase4n3-{n}: control {name} event {event:?} unowned");
                assert!(
                    !crate::screens::research::owns(event)
                        && !crate::screens::models::owns(event),
                    "phase4n3-{n}: {event:?} claimed by another seam"
                );
            }
        }
    }

    #[test]
    fn resume_gate_arms_then_refuses_without_the_typed_match() {
        let store = Store::new();
        store.domains.session.set_active(Some("dsflash:main".into()));
        assert!(perform("resume_confirm", &store).is_err(), "nothing selected");
        perform("resume.select.2", &store).expect("select");
        assert_eq!(store.domains.session.pending_resume("dsflash:main"), Some(2));
        let err = perform("resume_confirm", &store).expect_err("fail-closed");
        assert!(err.contains("exact-typed match"), "{err}");
    }
}
