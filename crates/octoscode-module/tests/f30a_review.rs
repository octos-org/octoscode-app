//! #30a — the board-3 review screens' gates:
//!
//! 1. **bindings coverage** — every one of the 36 declared ids resolves once
//!    a preview is folded (the #29c lesson: probe a FULL state; on an empty
//!    state `None` is correct — authored copy stays).
//! 2. **card-table agreement** — every copy id the table names exists in the
//!    authored `page.card`s.
//! 3. **the action table** — `diff.scope` cycles the pill; `review.start`
//!    maps the confirmed turn to `review/start` with the web's typed
//!    withholding reasons in precedence order (`native-review.ts:33-55`).
//! 4. **the preview fold** — a `diff/preview/get` result shaped by the
//!    octos-core types (`ui_protocol.rs:2700-2790`) folds into per-file
//!    +/− counts (`DiffReviewDialog.tsx:34-41`), 8 flattened lines with
//!    marks and gutter numbers.
//! 5. **replay** — the RECORDED r5-turn `review/start` traffic through a
//!    fake server: the request reaches the wire with the web's param shape
//!    (`history.ts:250`), and the recorded (null) reply is tolerated — the
//!    receipt folds only on `accepted: true`.

use std::sync::{Arc, Mutex};

use octoscode_module::bindings::{self, Ctx};
use octoscode_module::flow::FlowUi;
use octoscode_module::screens::review;
use octoscode_store::Store;

fn ctx() -> (Arc<Store>, Ctx<'static>) {
    let store: &'static Arc<Store> = Box::leak(Box::new(Arc::new(Store::new())));
    let ui: &'static Mutex<FlowUi> = Box::leak(Box::new(Mutex::new(FlowUi::default())));
    store.domains.session.set_active(Some("dsflash:main".into()));
    // A10: an open, connected Session is ready (`sessionNotReady` otherwise).
    store.set_connection("Live".into(), true);
    (store.clone(), Ctx::new(store, ui))
}

/// The recorded r5-turn wire shapes, cited in the asserts below.
const PREVIEW_JSON: &str = r##"{
  "status": "ready", "source": "pending_store",
  "preview": {"session_id": "dsflash:main", "preview_id": "01920000-0000-7000-8000-0000000000f1",
    "title": "Working tree",
    "files": [
      {"path": "crates/app/src/main.rs", "status": "modified", "hunks": [{"header": "@@",
        "lines": [
          {"kind": "context", "content": "fn main() {", "old_line": 1, "new_line": 1},
          {"kind": "removed", "content": "    run(old);", "old_line": 2},
          {"kind": "added", "content": "    run(new);", "new_line": 2},
          {"kind": "added", "content": "    check();", "new_line": 3}
        ]}]},
      {"path": "docs/b.md", "status": "added", "hunks": [{"header": "@@",
        "lines": [
          {"kind": "added", "content": "# b", "new_line": 1},
          {"kind": "context", "content": "", "new_line": 2}
        ]}]},
      {"path": "docs/c.md", "status": "deleted", "hunks": [{"header": "@@",
        "lines": [{"kind": "removed", "content": "gone", "old_line": 1}]}]},
      {"path": "docs/d.md", "status": "modified", "hunks": [{"header": "@@",
        "lines": [{"kind": "removed", "content": "x", "old_line": 9}]}]},
      {"path": "docs/e.md", "status": "modified", "hunks": [{"header": "@@",
        "lines": [{"kind": "added", "content": "y", "new_line": 9}]}]}
    ]}}"##;

fn seed_preview() {
    let v: serde_json::Value = serde_json::from_str(PREVIEW_JSON).unwrap();
    review::fold_preview(&v);
}

fn seed_run() {
    let mut ui = review::ui();
    ui.last_turn_id = Some("01920000-0000-7000-8000-0000000000a1".into());
    ui.preview_id = Some("01920000-0000-7000-8000-0000000000f1".into());
    ui.blocked = None;
    // The status slot resolves on the accepted receipt too
    // ("Reviewing · N specialists"); the coverage seed carries one.
    ui.agents = Some(3);
}

fn caps_for(store: &Store, caps: &[&str]) {
    store.set_capabilities(caps.iter().map(|c| c.to_string()).collect());
}

// ------------------------------------------------------- §1 bindings coverage

#[test]
fn every_declared_binding_resolves_on_a_folded_preview() {
    let _seq = review::test_lock();
    review::reset();
    let (store, ctx) = ctx();
    caps_for(&store, &["review/start", "review.start.v1"]);
    // A FULL window: the selected file fills all 8 row slots (2 ctx +
    // removed + added + 4 ctx), so every line/num/mark slot resolves. (A
    // shorter file HONESTLY leaves the tail slots authored — the per-file
    // model; the shorter case is asserted in the fold tests below.)
    let rows = serde_json::json!([
        {"kind": "context", "content": "use review;", "old_line": 1, "new_line": 1},
        {"kind": "context", "content": "mod x;", "old_line": 2, "new_line": 2},
        {"kind": "removed", "content": "old;", "old_line": 3},
        {"kind": "added", "content": "new;", "new_line": 3},
        {"kind": "context", "content": "a;", "old_line": 4, "new_line": 4},
        {"kind": "context", "content": "b;", "old_line": 5, "new_line": 5},
        {"kind": "context", "content": "c;", "old_line": 6, "new_line": 6},
        {"kind": "context", "content": "d;", "old_line": 7, "new_line": 7}
    ]);
    // The card declares THREE file rows: the selected file (full.rs, the
    // 8-row window above) plus two more, each with one changed line.
    review::fold_preview(&serde_json::json!({"preview": {"files": [
        {"path": "full.rs", "status": "modified",
         "hunks": [{"header": "@@", "lines": rows}]},
        {"path": "second.rs", "status": "modified",
         "hunks": [{"header": "@@", "lines": [
            {"kind": "added", "content": "two;", "new_line": 1}]}]},
        {"path": "third.rs", "status": "modified",
         "hunks": [{"header": "@@", "lines": [
            {"kind": "removed", "content": "three;", "old_line": 1}]}]}
    ]}}));
    seed_run();
    for (id, _desc) in review::BINDINGS {
        let v = bindings::query(&ctx, id);
        assert!(v.is_some(), "binding {id} does not resolve");
    }
    // the action ids are owned here and never reach the conversation router
    for (a, _d) in review::ACTIONS {
        assert!(review::owns_action(a) && bindings::is_action(a), "{a} not owned");
    }
}

#[test]
fn an_empty_state_keeps_the_authored_copy() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    // no preview folded: the live slots yield nothing (authored stays)…
    for id in ["review.add", "review.del", "review.file1.path", "review.line0", "review.mark2"] {
        assert!(bindings::query(&ctx, id).is_none(), "{id} should stay authored");
    }
    // …except the chrome the web always renders.
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Last turn ▾"));
    assert_eq!(bindings::query(&ctx, "review.start_label").unwrap(),
               serde_json::json!("Start native review"));
}

// --------------------------------------------------- §2 card-table agreement

#[test]
fn every_copy_id_exists_in_its_authored_card() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../design/stage-b/autonomy/cards");
    for (copy_id, binding) in review::COPY_SLOTS {
        let card = if *binding == "review.status"
            || *binding == "review.start_label"
            || binding.starts_with("review.finding")
        {
            "autonomy-02"
        } else {
            "autonomy-01"
        };
        let src = std::fs::read_to_string(root.join(card).join("page.card"))
            .expect("the committed Stage-B card");
        let needle = format!("copy {copy_id} {{");
        assert!(src.contains(&needle), "{card}: copy slot `{copy_id}` missing");
    }
}

// ---------------------------------------------------- §3 the action table

#[test]
fn diff_scope_cycles_the_pill() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Last turn ▾"));
    let e = review::resolve("diff.scope", 0, &ctx);
    assert!(matches!(e, review::Effect::ScopeCycle));
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Project ▾"));
    review::resolve("diff.scope", 0, &ctx);
    assert_eq!(bindings::query(&ctx, "review.scope").unwrap(),
               serde_json::json!("Last turn ▾"));
}

#[test]
fn start_maps_the_confirmed_turn_with_the_web_params() {
    let _seq = review::test_lock();
    review::reset();
    let (store, ctx) = ctx();
    caps_for(&store, &["review/start", "review.start.v1"]);
    seed_run();
    // A10: a review is a NEW turn — a fresh protocol UUID per start
    // (`native-review.ts:177` crypto.randomUUID()), never the last turn's id,
    // and no prompt unless instructions were typed (no fabricated default).
    let first = match review::resolve("review.start", 0, &ctx) {
        review::Effect::StartReview { session_id, turn_id, prompt } => {
            assert_eq!(session_id, "dsflash:main");
            assert_ne!(turn_id, "01920000-0000-7000-8000-0000000000a1", "not the confirmed turn");
            assert_eq!(turn_id.len(), 36, "a protocol UUID: {turn_id}");
            assert_eq!(prompt, None, "empty instructions send no prompt");
            turn_id
        }
        other => panic!("expected StartReview, got {other:?}"),
    };
    // Typed instructions ride trimmed and verbatim (inert: markup stays text).
    review::set_prompt("  <b>focus</b> on the parser  ");
    match review::resolve("review.start", 0, &ctx) {
        review::Effect::StartReview { turn_id, prompt, .. } => {
            assert_ne!(turn_id, first, "each start mints its own turn");
            assert_eq!(prompt.as_deref(), Some("<b>focus</b> on the parser"));
        }
        other => panic!("expected StartReview, got {other:?}"),
    }
    review::set_prompt("");
}

/// A10 — `sessionNotReady` is the Session's readiness (connected and open),
/// not the existence of an earlier turn.
#[test]
fn start_waits_for_a_ready_session_not_a_previous_turn() {
    let _seq = review::test_lock();
    review::reset();
    let (store, ctx) = ctx();
    caps_for(&store, &["review/start", "review.start.v1"]);
    // No turn has completed: still admitted.
    assert!(matches!(review::resolve("review.start", 0, &ctx), review::Effect::StartReview { .. }));
    store.set_connection("Reconnecting".into(), false);
    match review::resolve("review.start", 0, &ctx) {
        review::Effect::Blocked(r) => {
            assert_eq!(r, "Wait for this Session to finish recovery before starting review.")
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
    store.set_connection("Live".into(), true);
}

#[test]
fn start_is_blocked_with_the_typed_reasons_in_order() {
    let (store, ctx) = ctx();

    // capabilityUnsupported wins when the method OR feature is absent
    // (native-review.ts:18-24 gates on method AND feature).
    {
        let _seq = review::test_lock();
        review::reset();
        caps_for(&store, &["review/start"]); // feature missing
        seed_run();
        match review::resolve("review.start", 0, &ctx) {
            review::Effect::Blocked(r) => assert_eq!(
                r, "This server does not advertise native code review."),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
    // sessionBusy: a live turn owns the single turn slot (resolve takes the
    // FlowUi lock internally, so the seed drops its guard first).
    {
        let _seq = review::test_lock();
        review::reset();
        caps_for(&store, &["review/start", "review.start.v1"]);
        seed_run();
        ctx.ui.lock().unwrap().begin_turn_now("live-1");
        match review::resolve("review.start", 0, &ctx) {
            review::Effect::Blocked(r) => assert!(r.starts_with(
                "Wait for this Session's active turn"),
                "sessionBusy reason, got {r}"),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
    // sessionPendingInteraction: a pending approval must settle first.
    // Block 2's live turn is still active on this shared FlowUi — settle it
    // first, or the higher-precedence sessionBusy reason fires here instead.
    {
        let _seq = review::test_lock();
        review::reset();
        caps_for(&store, &["review/start", "review.start.v1"]);
        seed_run();
        ctx.ui.lock().unwrap().end_turn_for_test("live-1", true);
        ctx.ui.lock().unwrap().set_pending_for_test(true, false);
        match review::resolve("review.start", 0, &ctx) {
            review::Effect::Blocked(r) => assert!(r.starts_with(
                "Wait for this Session's pending questions and approvals"),
                "pending-interaction reason, got {r}"),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
}

#[test]
fn unhandled_ids_stay_unhandled() {
    let (_store, ctx) = ctx();
    assert!(matches!(
        review::resolve("composer.submit", 0, &ctx),
        review::Effect::Unhandled(_)
    ));
}

// ------------------------------------------------------------- §4 the fold

#[test]
fn the_preview_folds_counts_lines_marks_numbers() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();
    let st = review::ui();
    // per-file +/− over the file's hunk lines (DiffReviewDialog.tsx:34-41)
    assert_eq!(st.files.len(), 5);
    assert_eq!(st.files[0].path, "crates/app/src/main.rs");
    assert_eq!(st.files[0].add, 2, "file 1: two added lines");
    assert_eq!(st.files[0].del, 1, "file 1: one removed line");
    assert_eq!(st.files[2].del, 1, "deleted file counts its removals");
    // header totals: +4 −3 (across ALL files)
    let a: u64 = st.files.iter().map(|f| f.add).sum();
    let d: u64 = st.files.iter().map(|f| f.del).sum();
    assert_eq!((a, d), (4, 3));
    // the diff body renders the SELECTED file's real hunks (the first file
    // carrying a change), one row per line — file 1 has 4 rows, all fit.
    assert_eq!(st.selected_file.as_deref(), Some("crates/app/src/main.rs"));
    assert_eq!(st.lines.len(), 4);
    assert_eq!(st.hidden, 5, "9 preview rows − 4 shown = the real fold");
    assert_eq!(st.lines[0].content, "fn main() {");
    assert_eq!(st.lines[0].mark(), "", "context lines carry no mark");
    assert_eq!(st.lines[1].mark(), "-");
    assert_eq!(st.lines[2].mark(), "+");
    assert_eq!(st.lines[2].gutter(), "|2", "added takes the NEW column");
    assert_eq!(st.lines[1].gutter(), "2|", "removed keeps its OLD number");
    assert_eq!(st.lines[3].mark(), "+", "file 1's second added line");
}

#[test]
fn the_folded_values_reach_the_card_slots() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    seed_preview();
    assert_eq!(bindings::query(&ctx, "review.add").unwrap(), serde_json::json!("+4"));
    assert_eq!(bindings::query(&ctx, "review.del").unwrap(), serde_json::json!("-3"));
    assert_eq!(bindings::query(&ctx, "review.file1.path").unwrap(),
               serde_json::json!("crates/app/src/main.rs"));
    assert_eq!(bindings::query(&ctx, "review.file1.add").unwrap(), serde_json::json!("+2"));
    assert_eq!(bindings::query(&ctx, "review.line1").unwrap(), serde_json::json!("    run(old);"));
    assert_eq!(bindings::query(&ctx, "review.mark3").unwrap(), serde_json::json!("+"));
    assert_eq!(bindings::query(&ctx, "review.num3").unwrap(), serde_json::json!("|3"));
    // #30a2: header = the selected file; fold = the real count; findings stay
    // empty until a review result exists.
    assert_eq!(bindings::query(&ctx, "review.file_path").unwrap(),
               serde_json::json!("crates/app/src/main.rs"));
    assert_eq!(bindings::query(&ctx, "review.fold").unwrap(),
               serde_json::json!("⋮ 5 unmodified lines ⋮"));
    assert_eq!(bindings::query(&ctx, "review.finding_high.path").unwrap(),
               serde_json::json!(""));
}

#[test]
fn the_recorded_diff_preview_request_carries_the_core_params_shape() {
    // The committed r30a recording (a real a6ea8505 serve, zero model turns):
    // the screen's request on the wire matches the octos-core param type
    // (ui_protocol.rs:2483-2486) exactly, and the serve's typed answer to a
    // preview-less session (-32103 target-not-found) is the truth the
    // fixture pins — never fabricated.
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r30a-diffpreview-a6ea8505.jsonl");
    let frames = load(fixture);
    let req = frames
        .iter()
        .find(|f| f.dir == "out" && f.method == "diff/preview/get")
        .expect("the recording carries the screen's diff/preview/get");
    assert_eq!(
        req.body,
        serde_json::json!({"session_id": "dsflash:main",
                           "preview_id": "01920000-0000-7000-8000-0000000000f1"})
    );
}

#[test]
fn the_confirmed_turn_rides_the_real_terminal_envelope() {
    let _seq = review::test_lock();
    review::reset();
    // The REAL wire path (live-gate-a6ea8505.jsonl, seq 156, verbatim shape):
    // a projection/envelope whose payload.type is "turn_terminal". No fixture
    // ever carries a `turn/completed` method frame.
    let envelope: serde_json::Value = serde_json::from_str(
        r##"{"cursor": {"seq": 2468, "stream": "dsflash:main"},
             "payload": {"data": {"outcome": "completed",
                                  "token_usage": {"input_tokens": 164}},
                         "type": "turn_terminal"},
             "seq": 156, "session_id": "dsflash:main",
             "thread_id": "01a0e75b",
             "turn_id": "01a0e75b-dfb8-708a-a7ce-5d29c534f2f6"}"##,
    )
    .unwrap();
    review::note_envelope(&envelope);
    {
        let st = review::ui();
        assert_eq!(
            st.last_turn_id.as_deref(),
            Some("01a0e75b-dfb8-708a-a7ce-5d29c534f2f6")
        );
        // The fixtures carry no preview id on the record — the slot stays None.
        assert_eq!(st.preview_id, None);
    } // the STATE guard drops HERE (Drop lives to scope end, not last use)

    // A non-completed terminal never confirms the turn (outcome gate).
    review::reset();
    let interrupted: serde_json::Value = serde_json::from_str(
        r##"{"payload": {"data": {"outcome": "interrupted"}, "type": "turn_terminal"},
             "turn_id": "t-x"}"##,
    )
    .unwrap();
    review::note_envelope(&interrupted);
    assert!(review::ui().last_turn_id.is_none(), "interrupted confirms nothing");
}

#[test]
fn the_window_anchors_the_first_change_with_two_context_lines() {
    let _seq = review::test_lock();
    review::reset();
    // many-hunk case: a 14-line file whose only change sits at index 6 —
    // the 8-row window must START two context lines above it (the #28
    // diff-view changed-window shape).
    let lines: Vec<serde_json::Value> = (0..14)
        .map(|i| {
            if i == 6 {
                serde_json::json!({"kind": "added", "content": "new", "new_line": i + 1})
            } else {
                serde_json::json!({"kind": "context", "content": "c",
                                   "old_line": i + 1, "new_line": i + 1})
            }
        })
        .collect();
    let v = serde_json::json!({"preview": {"files": [
        {"path": "big.rs", "status": "modified",
         "hunks": [{"header": "@@", "lines": lines}]}]}});
    review::fold_preview(&v);
    let st = review::ui();
    assert_eq!(st.selected_file.as_deref(), Some("big.rs"));
    assert_eq!(st.lines.len(), 8);
    assert_eq!(st.hidden, 6);
    assert_eq!(st.lines[2].kind, "added", "the first change is IN the window");
    assert_eq!(st.lines[0].gutter(), "5|5", "context shows both columns");
}

#[test]
fn the_zero_and_one_hunk_states() {
    let _seq = review::test_lock();
    review::reset();
    // 0 hunks: no rows, no fold, no selected file -> the slots keep the
    // authored copy (the honest empty diff).
    review::fold_preview(&serde_json::json!({"preview": {"files": []}}));
    {
        // the STATE guard drops at THIS brace — bindings::query takes the
        // same lock inside review::query (std Mutex is not reentrant).
        let st = review::ui();
        assert!(st.lines.is_empty());
        assert_eq!(st.hidden, 0);
        assert!(st.selected_file.is_none());
    }
    let (_s, ctx) = ctx();
    assert!(bindings::query(&ctx, "review.file_path").is_none());
    assert_eq!(bindings::query(&ctx, "review.fold").unwrap(), serde_json::json!(""));
    // 1 hunk fitting the card: everything shows, nothing folds.
    review::fold_preview(&serde_json::json!({"preview": {"files": [
        {"path": "one.rs", "status": "modified", "hunks": [{"header": "@@", "lines": [
            {"kind": "removed", "content": "a", "old_line": 1},
            {"kind": "added", "content": "b", "new_line": 1}]}]}]}}));
    let st = review::ui();
    assert_eq!(st.lines.len(), 2);
    assert_eq!(st.hidden, 0);
    assert_eq!(st.selected_file.as_deref(), Some("one.rs"));
}

#[test]
fn the_diff_rows_rebuild_with_the_real_chips() {
    let _seq = review::test_lock();
    review::reset();
    review::fold_preview(&serde_json::from_str::<serde_json::Value>(PREVIEW_JSON).unwrap());
    let (card_src, _data, _kit) = review::lower_card_src("autonomy-01").expect("card");
    let (out, rows) = review::rebuild_diff_rows(&card_src);
    // file 1's window is [ctx, removed, added, added] -> chips on rows 1/2/3
    assert_eq!(rows.len(), 4, "one descriptor per emitted row: {rows:?}");
    // context rows carry their REAL char count too (the dl width sizing
    // needs it — no mid-token clipping): "fn main() {" is 11 chars.
    assert_eq!(rows[0], (0, "context".to_owned(), 11));
    assert_eq!(rows[1], (1, "removed".to_owned(), 13), "'    run(old);' chars");
    assert_eq!(rows[2], (2, "added".to_owned(), 13));
    assert_eq!(rows[3], (3, "added".to_owned(), 12));
    assert!(out.contains("Surface4db4e42a8189(instance: \"chip_1\")"),
            "removed row -> the design's removed chip");
    assert!(out.contains("Surface04a27fc304ff(instance: \"chip_2\")")
        && out.contains("Surface04a27fc304ff(instance: \"chip_3\")"),
        "added rows -> the design's added chip");
    assert!(out.contains("(instance: \"ln_0\", text: copy.ln_0_text)"),
            "context row -> the bare text pair");
    assert!(out.contains("text: \"-\"") && out.contains("text: \"+\""),
            "the +/- marks are baked on the changed rows");
    // 0-case: an empty preview rebuilds an EMPTY group, no chips
    review::reset();
    let (out0, rows0) = review::rebuild_diff_rows(&card_src);
    assert!(rows0.is_empty());
    let start = out0.find("Group3d2637879433").expect("the group stays");
    let seg = &out0[start..start + 140];
    assert!(seg.contains('}'), "empty group right after its open brace");
}

#[test]
fn the_gutter_never_duplicates_on_a_removed_added_pair() {
    let _seq = review::test_lock();
    review::reset();
    // The entry's case: a removed line followed by its added replacement.
    // The web renders old|new columns (DiffReviewDialog.tsx:152/:195/:161);
    // here each row prints ONE "old|new" string — never a duplicate.
    review::fold_preview(&serde_json::json!({"preview": {"files": [
        {"path": "pair.rs", "status": "modified", "hunks": [{"header": "@@", "lines": [
            {"kind": "context", "content": "a;", "old_line": 1, "new_line": 1},
            {"kind": "removed", "content": "old;", "old_line": 2},
            {"kind": "added", "content": "new;", "new_line": 2},
            {"kind": "context", "content": "b;", "old_line": 3, "new_line": 3}
        ]}]}]}}));
    let st = review::ui();
    let g: Vec<String> = st.lines.iter().map(|l| l.gutter()).collect();
    assert_eq!(g, vec!["1|1", "2|", "|2", "3|3"]);
    for i in 0..g.len() {
        for j in (i + 1)..g.len() {
            assert_ne!(g[i], g[j], "duplicate gutter string at rows {i}/{j}");
        }
    }
}

// --------------------------------------------------------------- §5 replay

struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn load(path: &str) -> Vec<Frame> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl ReplayServer {
    /// Serves the RECORDED reply body per method (r5-turn's review/start
    /// reply is `null` — the real recorded traffic), and logs every request.
    async fn start(canned: Vec<(String, serde_json::Value)>) -> Self {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::TcpListener;
        use tokio_tungstenite::tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
                rx.lock().unwrap().push((method.clone(), params));
                let body = canned
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(serde_json::Value::Null);
                let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": body});
                let _ = tx.lock().await.send(Message::Text(frame.to_string().into())).await;
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }
}

#[tokio::test]
async fn replay_the_recorded_review_start_reaches_the_wire() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r5-turn-a6ea8505.jsonl");
    let frames = load(fixture);
    let recorded = frames
        .iter()
        .find(|f| f.dir == "out" && f.method == "review/start")
        .expect("r5-turn records a review/start request")
        .body
        .clone();
    // The recording's params carry delivery "inline" (the web's constant).
    assert_eq!(recorded["delivery"], "inline");
    assert!(recorded["turn_id"].is_string(), "the recorded request names a turn");

    let reply = frames
        .iter()
        .find(|f| f.dir == "in" && f.method == "review/start")
        .map(|f| f.body.clone())
        .expect("r5-turn records the review/start reply");
    let server = ReplayServer::start(vec![("review/start".into(), reply)]).await;
    let (conv, mut _events) =
        octoscode_module::flow::Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");

    // The screen action maps to the recorded request shape (no prompt: the
    // recording typed none)…
    let turn = recorded["turn_id"].as_str().unwrap().to_owned();
    let effect = review::Effect::StartReview {
        session_id: conv.session_id(),
        turn_id: turn.clone(),
        prompt: None,
    };
    // The recording's reply is a server refusal (no receipt): the web's
    // `parseReviewStartResult` rejects it, so the start fails visibly.
    let err = review::perform(effect, &conv).await.expect_err("an unparseable receipt fails");
    assert!(err.starts_with("review/start:"), "{err}");

    // …and the server saw exactly the recorded params.
    let seen = server.received.lock().unwrap().clone();
    let (_m, params) = seen
        .iter()
        .find(|(m, _)| m == "review/start")
        .expect("the wire carried review/start");
    assert_eq!(params["delivery"], "inline");
    assert_eq!(params["turn_id"], recorded["turn_id"]);
    assert!(params.get("prompt").is_none(), "no fabricated prompt");
    // Nothing folds; the Session records the request and the failure.
    assert!(conv.store.domains.review.last_review().is_none());
    let notes: Vec<String> = conv
        .store
        .domains
        .session
        .timeline
        .entries(&conv.session_id())
        .into_iter()
        .filter(|e| e.turn_id.as_deref() == Some(turn.as_str()))
        .map(|e| e.text)
        .collect();
    assert_eq!(notes[0], "Native code review: Review requested for current project changes.");
    assert!(notes[1].starts_with("Native code review not started: "), "{notes:?}");
}
