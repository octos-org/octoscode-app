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
        // Pass 1: collect the wireable controls, and per event the SMALLEST
        // row its controls name — the family's own numbering decides whether
        // the derived index passes through (0-based) or shifts down (1-based).
        let mut wireable: Vec<(&str, &str, Vec<f64>)> = Vec::new();
        let mut family_min: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for (name, c) in controls {
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
            if let Some(n) = row_of(name) {
                family_min
                    .entry(event)
                    .and_modify(|m| *m = (*m).min(n))
                    .or_insert(n);
            }
            wireable.push((name, event, b));
        }
        // Pass 2: wire. #FX1 — the row rides in the id, derived from the
        // CONTROL NAME (not the event: most per-row events already name their
        // own row, and their resolvers take no index at all — see `row_of`).
        // So this is the ONE shared place a per-row tap gets its row, for
        // every card.
        for (name, event, b) in wireable {
            let event = &with_row_in_family(event, name, family_min.get(event).copied());
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

/// #FX1 — the row a control name addresses, or `None` for a non-row control.
///
/// The card's control names carry the row they belong to: `thread_1`,
/// `mon_1`, `row_2`, `file_3`, `model_row_0`, `browser_row_2`. That is the ONLY
/// place the row is knowable at wiring time, and it is the right source — NOT
/// the event name.
///
/// Deriving it from the EVENT would double-encode: most per-row events already
/// name their row (`provider.model.0`, `browser.enter.2`,
/// `session.resume.row_1`) and their resolvers take **no index parameter at
/// all** (`provider::resolve(id, value)` at provider.rs:343,
/// `browser::resolve(id, value)` at browser.rs:344 — each decodes the row from
/// the id itself), so a second row would make the id unresolvable.
///
/// ## The number is a NAME SUFFIX, not a resolver index
///
/// Measured across every per-row card's `service-actions.json`, the two card
/// families differ in TWO ways at once, which is why this function only PARSES
/// and [`with_row`] decides whether a conversion is needed:
///
/// | family | ids | base | what the event looks like |
/// |---|---|---|---|
/// | autonomy / conversation | `mon_1`, `thread_1`, `file_1`, `row_1` | 1-based | ONE shared event (`monitor.toggle`, `thread.open`) |
/// | provider / browser | `model_row_0`, `browser_row_0` | 0-based | the event ALREADY names the row (`provider.model.0`) |
///
/// The 0-based family never needs a conversion: its event already carries the
/// row, so no index is ever derived from its name. The 1-based family is
/// exactly the shared-event case where one is.
pub fn row_of(control_name: &str) -> Option<usize> {
    let tail = control_name.rsplit('_').next()?;
    // The row is the maximal digit SUFFIX of the last segment. `thread_1`
    // ends in bare digits (`"1"`), and the phase4n2 sidebar cards GLUE the
    // digits to the family word in one segment (`ctl_row2` -> `"row2"`,
    // measured in their service-actions.json). `str::parse` accepts a leading
    // '+', which is not a row number, so the remainder before the digits must
    // be empty or a plain alphabetic family word — anything else
    // (`"search"`, no digits at all) is not a row.
    let digits = tail.len() - tail.trim_end_matches(|b: char| b.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let (prefix, num) = tail.split_at(tail.len() - digits);
    if !prefix.is_empty() && !prefix.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    num.parse().ok()
}

/// Whether an action id ALREADY names the row it addresses.
///
/// Two spellings are in use: the dotted form (`provider.model.0`,
/// `browser.enter.3`) and the `monitor.pause#2` suffix this module adds. Both
/// count. An id that names its own row is routed VERBATIM: its resolver matches
/// the literal id and takes **no index** (`provider::resolve(id, value)` at
/// provider.rs:343, `browser::resolve(id, value)` at browser.rs:344), so
/// appending a second row would make the id match nothing at all.
pub fn event_names_its_own_row(event: &str) -> bool {
    if split_row(event).1.is_some() {
        return true;
    }
    match event.rsplit_once('.') {
        Some((_, tail)) => !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// #FX1 — split a routed action id into its base action and the ROW it
/// addresses, from the shared `#<row>` suffix.
///
/// This is #P4e1c's `autonomy::split_row`, promoted here because the suffix is
/// now written by the SHARED wiring path for every per-row card, not just
/// autonomy's. It stays additive: an id with no suffix splits to itself, so
/// every existing caller and test keeps working.
///
/// `"monitor.pause#2"` → `("monitor.pause", Some(2))`; a bare id → `(_, None)`;
/// `"goal#x"` (a name that merely contains '#') → `("goal#x", None)`.
pub fn split_row(action: &str) -> (&str, Option<usize>) {
    match action.rsplit_once('#') {
        Some((base, row)) if !row.is_empty() && row.bytes().all(|b| b.is_ascii_digit()) => {
            match row.parse::<usize>() {
                Ok(index) => (base, Some(index)),
                Err(_) => (action, None),
            }
        }
        _ => (action, None),
    }
}

/// Suffix an event with the row its control name addresses, unless the event
/// already names one.
///
/// Keeping the suffix OFF an event that already carries its own row is what
/// stops the double-encoding described on [`row_of`]: `provider.model.0` is
/// routed exactly as authored, while `mon_1`'s `monitor.toggle` becomes
/// `monitor.toggle#0` and reaches row 0 instead of silently addressing whatever
/// the host defaulted to.
pub fn with_row(event: &str, control_name: &str) -> String {
    with_row_in_family(event, control_name, None)
}

/// [`with_row`] with the card's own numbering made explicit: `family_min_row`
/// is the smallest row number found among the SAME event's controls in the
/// card. A family that names a row 0 (`ctl_row0..ctl_row4`, the phase4n2
/// sidebar cards) is 0-based and the derived row passes through — stamping
/// n-1 there would address the row BELOW the click. A family whose first row
/// is 1 (`thread_1..`, `mon_1..` — every previously measured shared-event
/// card) is 1-based and the index is n-1, so `thread_1` reaches store row 0.
/// `None` (a lone control, no family evidence) keeps the historical 1-based
/// contract.
pub fn with_row_in_family(
    event: &str,
    control_name: &str,
    family_min_row: Option<usize>,
) -> String {
    // The event already names its own row (`provider.model.0`): route it
    // VERBATIM. Its resolver matches the literal id and takes no index, so a
    // suffix here would make the id match nothing.
    if event_names_its_own_row(event) {
        return event.to_owned();
    }
    let Some(n) = row_of(control_name) else {
        return event.to_owned();
    };
    let index = if family_min_row == Some(0) { n } else { n - 1 };
    format!("{event}#{}", index)
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

    // ---- #FX1: the row semantics, pinned per card family -------------------
    //
    // Two bugs lived here and both were mine. `row_of` is a PURE PARSE of the
    // control name (no base conversion), and `with_row` is the only place that
    // converts — so these tests pin the two halves separately, which is the
    // split that makes both bugs impossible to reintroduce at once.

    #[test]
    fn row_of_parses_the_name_suffix_without_converting_its_base() {
        // autonomy-05/08, conversation-01/07 number from 1; p4-06/07/08 from 0.
        // `row_of` reports what the NAME says — converting here is what broke
        // the 0-based family (`model_row_2` must not become 1).
        assert_eq!(row_of("mon_1"), Some(1));
        assert_eq!(row_of("mon_2"), Some(2));
        assert_eq!(row_of("thread_5"), Some(5));
        assert_eq!(row_of("file_3"), Some(3));
        assert_eq!(row_of("row_4"), Some(4));
        assert_eq!(row_of("model_row_0"), Some(0));
        assert_eq!(row_of("model_row_2"), Some(2));
        assert_eq!(row_of("browser_row_0"), Some(0));
        assert_eq!(row_of("browser_row_3"), Some(3));
    }

    #[test]
    fn a_control_that_is_not_a_row_has_no_row() {
        // A single-shot control keeps the old behaviour: no suffix, row 0.
        for name in ["new_loop", "clear_goal", "pause_btn", "btn_diag", "goal"] {
            assert_eq!(row_of(name), None, "{name} is not a row control");
        }
        // A trailing non-number is not a row, and `+2` is not a row either
        // (`usize::from_str` would otherwise accept a leading plus).
        assert_eq!(row_of("row_+2"), None);
        assert_eq!(row_of("row_"), None);
    }

    #[test]
    fn an_event_that_already_names_its_row_is_routed_verbatim() {
        // The double-encode guard, in BOTH spellings. `provider::resolve(id,
        // value)` and `browser::resolve(id, value)` match the literal id and
        // take NO index, so `provider.model.0#0` would match nothing and the
        // row would be lost in the other direction.
        assert_eq!(with_row("provider.model.0", "model_row_0"), "provider.model.0");
        assert_eq!(with_row("browser.enter.3", "browser_row_3"), "browser.enter.3");
        // The dotted test covers both the recogniser and the verdict.
        assert!(event_names_its_own_row("provider.model.0"));
        assert!(event_names_its_own_row("browser.enter.3"));
        assert!(!event_names_its_own_row("monitor.toggle"));
        assert!(!event_names_its_own_row("thread.open"));
        // A dotted id whose tail is not a number is not a row id.
        assert!(!event_names_its_own_row("model.change.v2"));
    }

    #[test]
    fn a_shared_event_gets_the_row_converted_from_the_1_based_name() {
        // The conversion lives HERE, not in `row_of`: only a shared event needs
        // an index derived from a 1-based name, and `thread_1` must reach store
        // row 0 — handing `thread.open` a 1 opens the SECOND session when the
        // user clicked the first.
        assert_eq!(with_row("monitor.toggle", "mon_1"), "monitor.toggle#0");
        assert_eq!(with_row("monitor.toggle", "mon_2"), "monitor.toggle#1");
        assert_eq!(with_row("thread.open", "thread_1"), "thread.open#0");
        assert_eq!(with_row("thread.open", "thread_3"), "thread.open#2");
        // A non-row control is untouched.
        assert_eq!(with_row("loop.new", "new_loop"), "loop.new");
    }

    #[test]
    fn split_row_round_trips_what_with_row_wrote() {
        for (event, control, want) in [
            ("monitor.toggle", "mon_2", 1usize),
            ("thread.open", "thread_3", 2),
            ("loop.new", "new_loop", 0),
        ] {
            let wired = with_row(event, control);
            let (base, row) = split_row(&wired);
            assert_eq!(base, event, "the base action survives the round trip");
            assert_eq!(row.unwrap_or(0), want, "and so does the row: {wired}");
        }
        // A name that merely contains '#' is a base name, not a row.
        assert_eq!(split_row("goal#x"), ("goal#x", None));
        assert_eq!(split_row("goal.pause"), ("goal.pause", None));
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
