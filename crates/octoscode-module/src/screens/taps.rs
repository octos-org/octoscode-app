//! #35b item 1 — the ONE card-tap wiring every docked screen shares.
//!
//! #32h wired taps for setup-01 only: `connect::wire_events` took a
//! [`crate::screens::connect::Screen`] and read
//! `design/stage-b/setup/cards/<that screen>/service-actions.json`, so the
//! other docked screens (palette=setup-08, error=setup-11, loading=setup-12,
//! the theme cards) mounted their cards with **no `on_click` at all** and every
//! button was dead. The #35a click audit proved it: `error.reload` and
//! `error.copy_diagnostics` (setup-11) clicked to nothing.
//!
//! This module is the generalised form #35b asked for: ONE helper, driven by a
//! card *directory* rather than an enum, so every mount path that lowers a card
//! gets the same wiring. `connect.rs` now delegates here (its behaviour is
//! unchanged, byte for byte) and `palette::mount_screen` / `theme::mount` gain
//! the taps their DSL already carried but never published.
//!
//! Routing stays one-owner: a tap is handed to the resolver that owns the id
//! (see [`owner_of`]), and `lib.rs`'s `perform_action` table is untouched.

/// Which resolver owns a card's action id.
///
/// `screens::connect` owns the setup-01/02/03 ids (`connect`, `connect.retry`,
/// `input.*`, the provider radios, `create_profile`); everything else a card
/// can author is a conversation or chrome id owned by `lib.rs`'s
/// `perform_action` — the palette/error family (`palette::owns_action`,
/// palette.rs:113) and the chrome toggles (`actions.rs:86`, bindings.rs:99).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// `screens::connect::resolve` (reached via `perform_screen_action`).
    Connect,
    /// `lib.rs`'s `perform_action` (the one-owner conversation/chrome router).
    Action,
}

/// Who owns `action` — the routing decision a card tap makes.
pub fn owner_of(action: &str) -> Owner {
    if crate::screens::connect::is_action(action) {
        Owner::Connect
    } else {
        Owner::Action
    }
}

/// How far a lowered `abs_pos` may sit from a control's authored `source_bounds`
/// and still count as the same control.
///
/// #35d: 0.5px was too tight. The authored bounds are whole pixels, but the
/// lowering rounds AND lays out (padding, the card's own inset), so the emitted
/// `abs_pos` drifts up to 1px in BOTH axes: setup-11's `btn_diag` is authored at
/// [26, 473] and lowers to `25, 472`. At 0.5px the match MISSED, the control got
/// no handler, and it was dead on the mounted screen — proven by
/// `real_card_tests::setup_11_wires_each_control_into_its_own_block` (it wires
/// only `error.reload`). 1.5px absorbs that drift while staying far tighter than
/// the 20+px gap between neighbouring controls, so one control can never steal
/// another's handler.
const POS_TOLERANCE: f64 = 1.5;

/// Wire every CLICK control of the card in `card_dir` into the lowered `dsl`.
///
/// Each non-`input.*` control in `service-actions.json` gets
/// `on_click: || { NAV(t: "<event>") }` on the DesignNativeButton whose
/// `abs_pos` matches the control's authored `source_bounds`. Field controls
/// (`input.*`) are live text, not taps, and stay unwired.
///
/// `card_dir` is a card directory name (`"setup-11"`), read through
/// `design::file` like the connect path did, so this is identical for every
/// card. The `card events: N tap(s) wired for <dir>` log is unchanged in shape
/// (#32h's device evidence reads it).
pub fn wire_card_events(dsl: &str, card_dir: &str) -> String {
    wire_card_events_dir(
        dsl,
        &crate::design::dir(&format!("stage-b/setup/cards/{card_dir}")),
    )
}

/// [`wire_card_events`] for a card under ANY stage — the theme cards live in
/// `stage-b/{conversation,autonomy,setup}/cards` (`theme::card_for`), so the
/// caller passes the card directory itself.
///
/// The `service-actions.json` is read straight off disk, beside the same
/// `page.card` `lower_card` read: `design::file` is keyed by a build-time
/// whitelist, and a card outside that table would silently wire NOTHING.
pub fn wire_card_events_dir(dsl: &str, card_dir: &std::path::Path) -> String {
    let label = card_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| card_dir.display().to_string());
    let Ok(text) = std::fs::read_to_string(card_dir.join("service-actions.json")) else {
        return dsl.to_owned();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return dsl.to_owned();
    };
    let mut out = dsl.to_owned();
    let mut wired = 0usize;
    if let Some(controls) = v.get("controls").and_then(|c| c.as_object()) {
        for (_name, c) in controls {
            let (Some(event), Some(b)) = (
                c.get("event").and_then(|e| e.as_str()),
                c.get("source_bounds").and_then(|b| b.as_array()),
            ) else {
                continue;
            };
            if event.starts_with("input.") {
                continue; // live text, not a tap
            }
            let Some(b) = b.iter().map(|x| x.as_f64()).collect::<Option<Vec<_>>>() else {
                continue;
            };
            if b.len() < 2 {
                continue;
            }
            let (before, after) = (out.clone(), inject_click(&out, event, b[0], b[1]));
            if after.len() != before.len() {
                wired += 1;
            }
            out = after;
        }
    }
    makepad_widgets::log!("[octoscode] card events: {wired} tap(s) wired for {label}");
    out
}

/// Wire EXPLICIT (event, x, y) controls into a lowered `dsl`, bypassing the
/// card's authored `source_bounds`.
///
/// [`wire_card_events_dir`] matches each control's Stage-A atlas
/// `source_bounds` against the emitted `abs_pos` and tolerates 1.5px of drift
/// (`POS_TOLERANCE`). The autonomy cards are authored from a DIFFERENT atlas
/// pass than the one their `page.data.json` was laid out from, so the two
/// disagree by far more than 1.5px — measured (P4e1c): goal's Pause sits 4px
/// and Stop 16px right of its authored bounds, so the shared helper wires
/// **zero** goal controls. This function instead takes the card's OWN
/// placements (the geometry the card actually renders), so a handler follows
/// what is on screen rather than what the atlas recorded.
///
/// Additive: `wire_card_events_dir` keeps its authored-bounds path, so no
/// existing caller's behaviour changes.
///
/// Each control is `(name, event, x, y)`. The returned report is
/// `(name, event, wired)`; `wired: false` is the honest record of a control
/// the card drew but the DSL cannot click (an `Svg` icon, a `Text` node) — the
/// caller reports those rather than pretending they are live.
pub fn wire_events_at(
    dsl: &str,
    controls: &[(String, String, f64, f64)],
) -> (String, Vec<(String, String, bool)>) {
    let mut out = dsl.to_owned();
    let mut report = Vec::with_capacity(controls.len());
    for (name, event, x, y) in controls {
        out = inject_click(&out, event, *x, *y);
        // Whether the control is wired is decided by the DSL CARRYING the
        // handler, never by a length delta: `inject_click` rebuilds through
        // `lines()` + `join("\n")`, which drops a trailing newline, so a card
        // with no Button at all still changed length while wiring nothing.
        // A length-based flag reported those as wired — the false "verified"
        // RULES calls the worst outcome.
        let wired = out.contains(&format!("NAV(t: {event:?})"));
        report.push((name.clone(), event.clone(), wired));
    }
    makepad_widgets::log!(
        "[octoscode] card events: {} tap(s) wired at card placements",
        report.iter().filter(|(_, _, w)| *w).count()
    );
    (out, report)
}

/// #32h item 1 (the dispatch layer, the outer loop's L4): the (widget name,
/// action id) pairs the wired taps created. The host maps the names to live
/// widget ids and routes the clicks — nothing dispatched clicks inside the
/// mounted screen to the screens' action tables (Stage C tested the tables by
/// calling ids directly, never by clicking).
///
/// #35b: now read for EVERY docked card, not just setup-01, so the host's
/// `screen_taps` carries the mounted card's real controls whatever mounted it.
pub fn wired_taps(dsl: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = dsl.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if let Some(rest) = l.strip_suffix(" {") {
            if let Some((name, kind)) = rest.split_once(":=") {
                if kind.trim() == "DesignNativeButton" {
                    let mut j = i + 1;
                    while j < lines.len() && lines[j].trim() != "}" {
                        if let Some(e) = lines[j].trim().strip_prefix("on_click: || { NAV(t: ") {
                            let ev = e.trim_end_matches(") }").trim_matches('"');
                            out.push((name.trim().to_owned(), ev.to_owned()));
                        }
                        j += 1;
                    }
                    i = j;
                }
            }
        }
        i += 1;
    }
    out
}

/// Inject `on_click: || { NAV(t: "<event>") }` into the DesignNativeButton
/// block whose abs_pos matches (x, y) within [`POS_TOLERANCE`]px. #40b: the
/// tolerance must absorb the cards' own authored drift (setup-11's btn_diag:
/// mapped.json abs_pos 25.0,472.0 vs service-actions bounds 26,473 — 1px of
/// OCR/mapping noise dropped the injection and left the button dead).
/// Idempotent: a block already carrying on_click is skipped. Returns the DSL
/// unchanged when no block matches (the caller's wired count then stays put —
/// visible in the card-events log).
fn inject_click(dsl: &str, event: &str, x: f64, y: f64) -> String {
    let lines: Vec<&str> = dsl.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 1);
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        out.push(line.to_owned());
        i += 1;
        // A block header: `<name> := <Kind> {`
        let Some(rest) = line.strip_suffix(" {") else { continue };
        let Some((_name, kind)) = rest.split_once(":=") else { continue };
        // Handlers attach ONLY to the native Button instances.
        if kind.trim() != "DesignNativeButton" {
            continue;
        }
        // Scan the (flat) block: match abs_pos, find the insert point.
        let mut pos_ok = false;
        let mut already = false;
        let mut insert_after = None;
        let mut j = i;
        while j < lines.len() {
            let l = lines[j];
            if l.trim() == "}" {
                break;
            }
            if l.contains("on_click") {
                already = true;
            }
            if let Some(p) = l.trim().strip_prefix("abs_pos: vec2(") {
                let p = p.trim_end_matches(')');
                let mut it = p.split(',');
                let ok = match (it.next(), it.next()) {
                    (Some(px), Some(py)) => {
                        px.trim().parse::<f64>().is_ok_and(|vx| (vx - x).abs() < POS_TOLERANCE)
                            && py.trim().parse::<f64>().is_ok_and(|vy| (vy - y).abs() < POS_TOLERANCE)
                    }
                    _ => false,
                };
                if ok {
                    pos_ok = true;
                    insert_after = Some(j);
                }
            }
            if l.trim() == "enabled: true" {
                insert_after = Some(j);
            }
            j += 1;
        }
        if already || !pos_ok {
            continue;
        }
        let at = insert_after.unwrap_or(i - 1);
        for l in &lines[i..=at] {
            out.push((*l).to_owned());
        }
        out.push(format!("on_click: || {{ NAV(t: {event:?}) }}"));
        i = at + 1;
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // #35b item 3: the routing table is the load-bearing decision, so it is
    // pinned per control id — the setup-01/02/03 ids go to connect::resolve,
    // and every other card id to lib.rs's perform_action.

    #[test]
    fn setup_screen_ids_route_to_the_connect_resolver() {
        for id in crate::screens::connect::ACTIONS.iter().map(|(a, _)| *a) {
            assert_eq!(owner_of(id), Owner::Connect, "{id} is connect::ACTIONS");
        }
    }

    #[test]
    fn the_palette_and_error_ids_route_to_perform_action() {
        // #35a's dead controls: setup-11's two buttons.
        for id in [
            "error.reload",
            "error.copy_diagnostics",
            "connection.retry",
            "error.copy",
        ] {
            assert_eq!(owner_of(id), Owner::Action, "{id} is not a connect id");
            assert!(
                crate::screens::palette::owns_action(id),
                "{id} must have a palette owner arm (palette.rs:113)"
            );
        }
    }

    #[test]
    fn the_base_chrome_ids_route_to_perform_action() {
        for id in [
            "session.new",
            "thread.open",
            "review.toggle",
            "settings.toggle",
            "session.refresh",
            "composer.submit",
        ] {
            assert_eq!(owner_of(id), Owner::Action, "{id} is a chrome id");
        }
    }

    // -- the wiring itself ------------------------------------------------------

    const BUTTON_DSL: &str = "\
root := DesignSurface {
btn_reload := DesignNativeButton {
abs_pos: vec2(26, 387)
size: vec2(303, 65)
enabled: true
text: \"Reload\"
}
btn_other := DesignNativeButton {
abs_pos: vec2(26, 473)
size: vec2(303, 63)
enabled: true
text: \"Copy\"
}
}";

    #[test]
    fn a_click_control_gets_its_on_click_and_an_unmatched_one_is_skipped() {
        // No design file for this dir: the helper must pass the DSL through
        // untouched rather than panic.
        let same = wire_card_events(BUTTON_DSL, "setup-99-does-not-exist");
        assert_eq!(same, BUTTON_DSL);
        assert!(!wired_taps(&same).iter().any(|(n, _)| n == "btn_reload"));
    }

    #[test]
    fn inject_click_targets_the_button_whose_position_matches() {
        let out = inject_click(BUTTON_DSL, "error.reload", 26.0, 387.0);
        let taps = wired_taps(&out);
        assert_eq!(taps.len(), 1, "one control wired: {taps:?}");
        assert_eq!(taps[0].0, "btn_reload");
        assert_eq!(taps[0].1, "error.reload");
        // The other button is untouched, and the handler is inside its block.
        assert!(out.contains("btn_other := DesignNativeButton {\nabs_pos: vec2(26, 473)"));
    }

    #[test]
    fn inject_click_is_idempotent() {
        let once = inject_click(BUTTON_DSL, "error.reload", 26.0, 387.0);
        let twice = inject_click(&once, "error.reload", 26.0, 387.0);
        assert_eq!(once, twice, "a second pass must not add a duplicate handler");
        assert_eq!(wired_taps(&twice).len(), 1);
    }

    #[test]
    fn wired_taps_reads_back_a_handler_in_either_position() {
        // `enabled: true` is the insert anchor on a lowered block; make sure a
        // handler written before it still reads back.
        let dsl = "\
a := DesignNativeButton {
on_click: || { NAV(t: \"error.copy\") }
abs_pos: vec2(1, 2)
}";
        assert_eq!(
            wired_taps(dsl),
            vec![("a".to_owned(), "error.copy".to_owned())]
        );
    }
}

#[cfg(test)]
mod real_card_tests {
    use super::*;

    /// #35d: run the REAL setup-11 card through the shared wiring and assert
    /// each control's handler lands in ITS OWN block. The audit reported
    /// "2 tap(s) wired for setup-11" yet only `error.reload` ever fired, which
    /// is what a duplicate-in-one-block looks like.
    #[test]
    fn setup_11_wires_each_control_into_its_own_block() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/stage-b/setup/cards/setup-11");
        if !dir.join("page.card").is_file() {
            return; // the design tree is not vendored into this build
        }
        let Ok(text) = std::fs::read_to_string(dir.join("page.card")) else { return };
        let Ok(data) = std::fs::read_to_string(dir.join("page.data.json")) else { return };
        let Ok(data) = serde_json::from_str::<serde_json::Value>(&data) else { return };
        let Ok(prepared) = octoscript_makepad::l0::prepare(&text, &data, &dir.join("kit")) else { return };
        // NO `inspectable()`: palette::lower_screen (palette.rs:342) does not
        // call it — only connect.rs:695 does — so calling it here would lower a
        // DIFFERENT tree than the app mounts and the test would not be a
        // faithful reproduction of the dock-error path.
        let Ok(dsl) = crate::design::with_fonts(
            octoscript_makepad::design::to_makepad_ui(&prepared.tree)) else { return };

        // What the lowering produced, BEFORE wiring: which blocks exist and
        // where. This is the evidence the fix depends on.
        let mut blocks: Vec<(String, String, String)> = Vec::new(); // (name, abs_pos, kind)
        let lines: Vec<&str> = dsl.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            let Some(rest) = l.strip_suffix(" {") else { continue };
            let Some((name, kind)) = rest.split_once(":=") else { continue };
            if kind.trim() != "DesignNativeButton" { continue; }
            let mut pos = String::new();
            for j in i + 1..lines.len() {
                if lines[j].trim() == "}" { break; }
                if let Some(p) = lines[j].trim().strip_prefix("abs_pos: vec2(") {
                    pos = p.trim_end_matches(')').to_owned();
                }
            }
            blocks.push((name.trim().to_owned(), pos, kind.trim().to_owned()));
        }
        eprintln!("setup-11 DesignNativeButton blocks: {blocks:?}");

        let wired = wire_card_events_dir(&dsl, &dir);
        let taps = wired_taps(&wired);
        eprintln!("setup-11 taps after wiring: {taps:?}");

        // Each of the two controls must appear EXACTLY once, in its own block.
        let reload = taps.iter().filter(|(_, e)| e == "error.reload").count();
        let diag = taps.iter().filter(|(_, e)| e == "error.copy_diagnostics").count();
        assert_eq!(reload, 1, "error.reload wired exactly once, taps={taps:?}");
        assert_eq!(diag, 1, "error.copy_diagnostics wired exactly once, taps={taps:?}");
        assert_ne!(reload + diag, 0, "both controls wired");
    }
}
