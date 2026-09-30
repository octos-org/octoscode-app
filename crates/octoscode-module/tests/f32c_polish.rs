//! Entry #32c — board-3 list polish (backlog items 7, 10, 11). RED-first:
//! every assertion below FAILED on the pre-#32c code (the run is quoted in
//! `.peer/report-32c.md`), pinning the fix contract:
//! - item 7: loops/monitors row text + icons render ~10% larger (atlas size);
//!   commands ellipsize ONLY when the rendered string would overflow the slot.
//! - item 10: a paused monitor row shows the RESUME (play) icon.
//! - item 11: fleet 0 peers → compact empty card, no sample goal heading;
//!   rows carry the `elapsed · tokens` meta line; Done wears the web's
//!   terminal grey (`--dsw-alias-label-secondary` #61666b, theme.css:82);
//!   tasks_card sizes to its content (no fixed 648 height); a running task
//!   without output shows "Waiting for output…", never the design's samples.

use std::sync::{Arc, Mutex};

use octoscode_module::bindings::Ctx;
use octoscode_module::flow::FlowUi;
use octoscode_module::screens::autonomy as au;
use octoscode_module::screens::fleet;
use octoscode_store::Store;

// ----------------------------------------------------------------- item 7+10

fn monitor(k: usize, status: &str, argv: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "monitor_id": format!("monitor_0{k}"),
        "name": format!("probe {k}"),
        "argv": argv,
        "status": status,
        "interval_seconds": 3600,
    })
}

/// A genuinely-fitting argv: 10 chars ≈ 70px at the scaled mono size —
/// comfortably inside the 114.88px mon_1_cmd slot, so no ellipsis may appear
/// (the old fixed ellipsize(…, 13) also left it verbatim; the guard pins the
/// width-aware path against regressing to a fixed cap).
const FITTING_ARGV: &[&str] = &["./watch.sh"];

#[test]
fn monitor_rows_render_atlas_sized_and_ellipsize_only_when_needed() {
    let st = au::AutonomyState {
        monitors: vec![
            monitor(1, "active", FITTING_ARGV),
            monitor(2, "paused", &["/bin/echo", "a-much-longer-command-that-truly-overflows"]),
        ],
        ..Default::default()
    };
    let l = au::lower_tree(au::Screen3::Monitors, &st).expect("monitors lowers");

    // Item 10: the PAUSED row shows the resume (play) icon — the design's own
    // play asset (autonomy-04 has it; autonomy-05's authored rows only ever
    // pause) — while the ACTIVE row keeps pause.
    assert!(
        l.dsl.contains("loop_1_play"),
        "the paused row must reference the play (resume) asset"
    );
    if let Some(i) = l.dsl.find("mon_2_pause-") {
        eprintln!(
            "RESIDUAL[{}..]: {:?}",
            i,
            &l.dsl[i.saturating_sub(160)..(i + 160).min(l.dsl.len())]
        );
    }
    assert!(
        !l.dsl.contains("mon_2_pause-"),
        "the paused row must not keep the pause icon"
    );
    assert!(
        l.dsl.contains("mon_1_pause-"),
        "the active row keeps its pause icon"
    );

    // Item 7: the icon nodes render 10% larger (24 → 26.4), re-centred.
    let (w, _) = icon_size(&l);
    assert!((w - 26.4).abs() < 0.01, "icon nodes scale x1.1, got {w}");

    // Item 7: the row text renders at the scaled size — 14.21sp cmd →
    // 15.63sp → font_size 15.63 * 0.75 = 11.72px in the DSL
    // (`design.rs:387` writes `size * 0.75`).
    assert!(
        l.dsl.contains("11.72"),
        "the mon cmd text must render at the x1.1 size (11.72px), dsl: {}",
        snippet(&l.dsl, "11.")
    );

    // Item 7: width-aware ellipsis — the 18-char argv FITS its slot, so the
    // command renders verbatim (the old fixed ellipsize(…, 13) chopped it to
    // "./scripts/wat…"). The truly-overflowing row 2 still ellipsizes.
    let cmd1 = node_text(&l, "mon_1_cmd");
    assert_eq!(cmd1, "./watch.sh", "a fitting command is never truncated");
    let cmd2 = node_text(&l, "mon_2_cmd");
    assert!(cmd2.ends_with('…'), "an overflowing command still ellipsizes");
}

fn icon_size(l: &au::Lowered) -> (f64, f64) {
    let (_, w, h) = l
        .measured
        .iter()
        .find(|(id, _)| id.contains("_pause"))
        .expect("a pause icon node")
        .1.clone();
    let _ = h;
    (w, h)
}

fn node_text(l: &au::Lowered, id: &str) -> String {
    // the inventory keys nodes by their ORIGINAL card id (`l0::inspectable`;
    // f30b's `at()` precedent reads `original_id`).
    l.inventory
        .iter()
        .find(|n| n["original_id"] == serde_json::json!(id))
        .map(|n| n["text"].as_str().unwrap_or_default().to_owned())
        .unwrap_or_default()
}

fn snippet(hay: &str, needle: &str) -> String {
    match hay.find(needle) {
        Some(i) => hay[i.saturating_sub(40)..(i + 60).min(hay.len())].to_owned(),
        None => "<absent>".to_owned(),
    }
}

#[test]
fn loop_rows_render_atlas_sized_text() {
    let st = au::AutonomyState {
        loops: vec![
            serde_json::json!({
                "loop_id": "loop_01", "prompt": "r1 replay probe",
                "mode": "fixed_interval", "interval_seconds": 3600, "status": "active",
            });
            3
        ],
        ..Default::default()
    };
    let l = au::lower_tree(au::Screen3::Loops, &st).expect("loops lowers");
    // 15sp name → 16.5sp → 12.375px (`size * 0.75`); the authored 11.25px was
    // the ~10%-too-small render the backlog flags.
    assert!(
        l.dsl.contains("12.375"),
        "loop names render at the x1.1 size (12.375px)"
    );
}

// ------------------------------------------------------------------- item 11

fn peers_store(rows: &[(&str, bool)]) -> Arc<Store> {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    for (name, closed) in rows {
        store.domains.peer.stage((*name).to_owned());
        if *closed {
            store.domains.peer.mark_closed(name);
        }
    }
    store
}

fn tasks_store(running: &[(&str, &str)], done: &[&str]) -> Arc<Store> {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    for (i, (label, line)) in running.iter().enumerate() {
        let snap = octoscode_store::domains::task::TaskSnapshot::from_list_row(
            format!("run-{i}"),
            (*label).to_owned(),
            "running".to_owned(),
            "running".to_owned(),
            Some((*label).to_owned()),
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        );
        store.domains.task.upsert_snapshot(snap);
        if !line.is_empty() {
            store
                .domains
                .task
                .append_output(&format!("run-{i}"), &format!("{line}\n"));
        }
    }
    for (i, label) in done.iter().enumerate() {
        let snap = octoscode_store::domains::task::TaskSnapshot::from_list_row(
            format!("done-{i}"),
            (*label).to_owned(),
            "done".to_owned(),
            "done".to_owned(),
            Some((*label).to_owned()),
            None,
            None,
            None,
            0,
            Vec::new(),
            None,
            None,
        );
        store.domains.task.upsert_snapshot(snap);
    }
    store
}

#[test]
fn fleet_zero_peers_is_a_compact_empty_card_without_the_sample_goal() {
    let ui = Mutex::new(FlowUi::default());
    let store = peers_store(&[]);
    let ctx = Ctx::new(&store, &ui);
    let (src, data, _) = fleet::lower_card_src("autonomy-06", &ctx).expect("fleet lowers");

    // The design's sample goal heading must go with the rows.
    assert!(
        !src.contains("Fix steer queue"),
        "0 peers must not render the sample goal heading"
    );
    let pl = data["$kit"]["placements"].as_object().unwrap();
    assert!(
        !pl.contains_key("fleet_goal") && !pl.contains_key("fleet_goal_label"),
        "the goal group placements drop with the rows"
    );
    // The card compacts around the empty line (authored 389 → the empty text
    // bottom 264 + the authored ~11px pad − card y 217 ≈ 58): compacted, far
    // from the authored full height, exact geometry verified on the capture.
    let h = pl["fleet_card"]["layout"]["h"].as_f64().unwrap();
    assert!((40.0..120.0).contains(&h), "fleet_card compacts (≈58), got {h}");
    assert!(src.contains("No peers yet"));
}

#[test]
fn fleet_rows_carry_the_elapsed_tokens_meta_line_and_done_is_grey() {
    let ui = Mutex::new(FlowUi::default());
    let store = peers_store(&[("tests", false), ("docs", false), ("review", true)]);
    let ctx = Ctx::new(&store, &ui);
    let (src, _data, _) = fleet::lower_card_src("autonomy-06", &ctx).expect("fleet lowers");

    // Every generated row mints its meta slot (elapsed · tokens).
    for i in 0..3 {
        assert!(
            src.contains(&format!("copy.peer_r{i}_meta_text")),
            "row {i} carries the meta slot"
        );
    }
    // tokens has no store source → the web's own zero-value dash
    // (`FleetView.tsx` renderFleetRow: `peer.tokens > 0 ? … : " · —"`).
    assert!(
        src.contains("· —"),
        "the meta line shows the web's dash for token-less rows"
    );
    // Done wears the web's terminal grey (#61666b, theme.css:82) — visible
    // only in the fully-lowered DSL (kit tokens resolve there).
    let dsl = fleet::lower("autonomy-06", &ctx).expect("fleet lowers");
    assert!(
        dsl.contains("61666b"),
        "the Done badge text uses the web's terminal grey, got: {}",
        snippet(&dsl, "Done")
    );
}

#[test]
fn tasks_card_sizes_to_its_content_and_empty_output_waits() {
    let ui = Mutex::new(FlowUi::default());

    // 1 running (WITH output) + 2 done → the card grows to the content:
    // run block 265..652, done rows 668..717 and 724..773 → content bottom
    // 773 + authored pad 17 (756−739) − card y 108 = 682; the fixed 648 is
    // gone (and 3 items no longer overflow it).
    let store = tasks_store(&[("cargo test -p octos-cli", "running 12 tests …")], &["cargo clippy", "cargo fmt"]);
    let ctx = Ctx::new(&store, &ui);
    let (src, data, _) = fleet::lower_card_src("autonomy-07", &ctx).expect("tasks lowers");
    let pl = data["$kit"]["placements"].as_object().unwrap();
    let h = pl["tasks_card"]["layout"]["h"].as_f64().unwrap();
    assert!((h - 682.0).abs() < 0.5, "tasks_card sizes to content (682), got {h}");

    // A running task whose output has not arrived shows the waiting line —
    // never the design's sample log lines.
    let store = tasks_store(&[("cargo build", "")], &[]);
    let ctx = Ctx::new(&store, &ui);
    let (src, _, _) = fleet::lower_card_src("autonomy-07", &ctx).expect("tasks lowers");
    assert!(
        src.contains("Waiting for output"),
        "a no-output running task shows the waiting line"
    );
    assert!(
        !src.contains("text: copy.t_log0_text"),
        "generated blocks never reference the authored sample-log slots"
    );
}
