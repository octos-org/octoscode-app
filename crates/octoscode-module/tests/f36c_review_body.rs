//! Entry #36c — the Review sheet's body was empty although the header showed
//! "+62 −5".
//!
//! **Measured on main, live** (`OCTOSCODE_CHROME=review`, the real `OctoscodeView`
//! shell through `keys_probe`, `/snap?all=1`, 162 entries): 10 `review*` widgets,
//! `review_panel` 748 px tall with an empty body below y=70, and the only
//! `file/row/diff/hunk` id in the whole tree is `timeline_list` (the conversation,
//! not the review). So the rows were **absent, not zero-height** — the branch
//! item 1 asks us to distinguish. Root cause `lib.rs:529-532`:
//! *"Empty for now — the header + scope pill only."*
//!
//! The fix mounts the body: a `PortalList` of file rows and one of the selected
//! file's diff lines, fed from the screen cache through the same
//! `bindings::query` path the other lists use (`bindings.rs:172-173` routes
//! `review::query`; the pattern is `lib.rs:2164-2183`).
//!
//! The web reference is `DiffReviewDialog.tsx:125-146`
//! (`preview.files.map(...)` as `<details className="diff-file">` rows with hunks
//! beneath) and its explicit empty state at `:120-122` (`review-empty`), which is
//! why this test asserts rows for a SEEDED preview and zero rows without one.
//!
//! Slot counts are fixed by the card's own bindings — three files
//! (`review.file1..3.*`) and eight lines (`review.line0..7` / `review.num0..7`) —
//! so the windows here are bounded, never unbounded.

use std::sync::{Arc, Mutex};

use octoscode_module::bindings::{self, Ctx};
use octoscode_module::flow::FlowUi;
use octoscode_module::screens::review;

fn ctx() -> (Arc<octoscode_store::Store>, Ctx<'static>) {
    let store: &'static Arc<octoscode_store::Store> =
        Box::leak(Box::new(Arc::new(octoscode_store::Store::new())));
    let ui: &'static Mutex<FlowUi> = Box::leak(Box::new(Mutex::new(FlowUi::default())));
    store.domains.session.set_active(Some("dsflash:main".into()));
    (store.clone(), Ctx::new(store, ui))
}

/// The recorded r5-turn wire shapes (the same fixture f30a folds), cited below.
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
        "lines": [{"kind": "removed", "content": "gone", "old_line": 1}]}]}
    ]}}"##;

fn seed_preview() {
    let v: serde_json::Value = serde_json::from_str(PREVIEW_JSON).unwrap();
    review::fold_preview(&v);
}

fn s(ctx: &Ctx<'_>, id: &str) -> String {
    bindings::query(ctx, id)
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The file rows the mounted body renders, in slot order. The window is the
/// three bound slots, truncated at the first empty path.
fn file_rows(ctx: &Ctx<'_>) -> Vec<(String, String, String)> {
    (1..=3)
        .map(|n| {
            (
                s(ctx, &format!("review.file{n}.path")),
                s(ctx, &format!("review.file{n}.add")),
                s(ctx, &format!("review.file{n}.del")),
            )
        })
        .take_while(|(path, _, _)| !path.is_empty())
        .collect()
}

/// The diff lines the mounted body renders: the eight bound slots, windowed to
/// the last non-empty one (a short preview shows short, not blank filler).
fn diff_lines(ctx: &Ctx<'_>) -> Vec<(String, String)> {
    let all: Vec<(String, String)> = (0..8)
        .map(|i| (s(ctx, &format!("review.line{i}")), s(ctx, &format!("review.num{i}"))))
        .collect();
    let last = all
        .iter()
        .rposition(|(text, _)| !text.is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    all[..last].to_vec()
}

/// THE fails-on-main test. Read this before the others.
///
/// The five tests above pass on **main** too, and that is the finding, not a
/// defect in them: the row DATA was never the problem. `review::query`
/// (`review.rs:349-357`) already resolved `review.file{N}.path/add/del` and
/// `review.line{N}` from the cache — the f30a gates prove that — but nothing in
/// the SHELL ever mounted them. The live `/snap` on main shows it: 162 entries,
/// 10 `review*` widgets, header + scope pill + close and **no row widget at
/// any rect**. So the bug was purely the missing MOUNT, and a test that only
/// queries the data cannot fail on main.
///
/// This one therefore asserts the mount itself — the shell's script body, read
/// from source — and it is the assert that fails on main (the strings are
/// absent there). The live numbers that pair with it, measured through the
/// instrument on the real shell (`OCTOSCODE_CHROME=review`, `/snap?all=1`):
///
/// | | main | after |
/// |---|---|---|
/// | entries | 162 | 188 |
/// | `review_files` | absent | `[0, 76, 360, 132]` |
/// | `review_diff` | absent | `[0, 214, 360, 372]` |
/// | both inside `review_panel` `[0,32,360,748]` | n/a | yes |
#[test]
fn the_shell_mounts_the_review_body() {
    let shell = include_str!("../src/lib.rs");
    // The body lists and their row templates…
    assert!(
        shell.contains("review_files := PortalList {"),
        "the review panel must mount a file-row PortalList (main: header only)"
    );
    assert!(
        shell.contains("review_diff := PortalList {"),
        "the review panel must mount a diff PortalList (main: header only)"
    );
    assert!(
        shell.contains("ReviewFileRowTpl") && shell.contains("ReviewLineRowTpl"),
        "both lists need their row templates"
    );
    // …and the draw_walk arms that feed them from the cache.
    assert!(
        shell.contains("uid == review_files_uid"),
        "draw_walk must feed the file rows (the palette arm is the precedent)"
    );
    assert!(
        shell.contains("uid == review_diff_uid"),
        "draw_walk must feed the diff lines"
    );
    assert!(
        shell.contains("ids!(review_file_path)).set_text(cx, path)"),
        "the file path must be set from the cache, not left authored"
    );
    // The old hand-written "Empty for now" note must be gone, or the next
    // reader assumes the body is intentionally absent.
    assert!(
        !shell.contains("Empty for now"),
        "the stale header-only note must be removed with the fix"
    );
}

/// ITEM 1/2 core: with a preview folded, the file rows and the selected file's
/// diff MUST resolve. This is the CONSUMER side of the fix: the data resolved on
/// main, so this guards against a future regression in the cache fold that would
/// leave a mounted-but-blank body (exactly the live symptom).
#[test]
fn a_seeded_preview_yields_file_rows_and_diff_lines() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();
    let (_store, ctx) = ctx();

    let files = file_rows(&ctx);
    assert!(
        files.len() >= 2,
        "a preview with 3 files must yield at least 2 file rows, got {files:?}"
    );
    assert_eq!(
        files[0].0, "crates/app/src/main.rs",
        "the first row is the first file's path (DiffReviewDialog.tsx:133)"
    );
    assert!(
        files[0].1.starts_with('+') && files[0].2.starts_with('-'),
        "each row carries its +/- counts (row 0 was {:?})",
        files[0]
    );
    assert!(
        files.iter().any(|f| f.0 == "docs/b.md"),
        "the rows follow document order, not just the first file: {files:?}"
    );

    let lines = diff_lines(&ctx);
    assert!(
        lines.len() >= 4,
        "the first file's diff must render at least 4 lines, got {}: {lines:?}",
        lines.len()
    );
    assert!(
        lines[0].0.contains("fn main()"),
        "the first diff line is the hunk's first content, got {:?}",
        lines[0]
    );
    assert!(
        lines.iter().any(|(t, _)| t.contains("run(new)")),
        "an added line must be present: {lines:?}"
    );

    // The header totals are what the panel SHOWS; they must agree with the rows
    // the body renders (the web sums the same preview, :34-41). This fixture's
    // added lines are main.rs x2 (`run(new)`, `check()`), b.md x1 (`# b`),
    // c.md x0 -> +3; removed are main.rs x1 (`run(old)`), c.md x1 (`gone`) -> -2.
    let add = s(&ctx, "review.add");
    let del = s(&ctx, "review.del");
    assert_eq!(add, "+3", "the + total over the folded preview");
    assert_eq!(del, "-2", "the - total over the folded preview");
}

/// No preview → no rows, which is web-correct (`review-empty`,
/// DiffReviewDialog.tsx:120-122). This is the empty-state half of item 2 and it
/// must hold on BOTH sides, so it is the one assert that does not change.
#[test]
fn no_preview_yields_no_rows() {
    let _seq = review::test_lock();
    review::reset();
    let (_store, ctx) = ctx();
    assert!(
        file_rows(&ctx).is_empty(),
        "with no preview folded there are no file rows to render"
    );
    assert!(
        diff_lines(&ctx).is_empty(),
        "with no preview folded there are no diff lines to render"
    );
}

/// The slot windows are BOUNDED by the card's own bindings, never unbounded:
/// three file slots, eight line slots. A longer preview must not grow the
/// window (the card's rows are fixed chrome; the rest is the "N unmodified"
/// fold count).
#[test]
fn the_row_windows_are_bounded_by_the_card_slots() {
    let _seq = review::test_lock();
    review::reset();
    // A preview with far more files/lines than the card has slots.
    let many: serde_json::Value = serde_json::from_str(
        r#"{"preview":{"files":[
            {"path":"a.rs","hunks":[{"header":"@@","lines":[
              {"kind":"added","content":"1","new_line":1},{"kind":"added","content":"2","new_line":2},
              {"kind":"added","content":"3","new_line":3},{"kind":"added","content":"4","new_line":4},
              {"kind":"added","content":"5","new_line":5},{"kind":"added","content":"6","new_line":6},
              {"kind":"added","content":"7","new_line":7},{"kind":"added","content":"8","new_line":8},
              {"kind":"added","content":"9","new_line":9},{"kind":"added","content":"10","new_line":10}
            ]}]},
            {"path":"b.rs","hunks":[{"header":"@@","lines":[{"kind":"added","content":"x","new_line":1}]}]},
            {"path":"c.rs","hunks":[{"header":"@@","lines":[{"kind":"added","content":"y","new_line":1}]}]},
            {"path":"d.rs","hunks":[{"header":"@@","lines":[{"kind":"added","content":"z","new_line":1}]}]}
        ]}}"#,
    )
    .unwrap();
    review::fold_preview(&many);
    let (_store, ctx) = ctx();
    assert_eq!(
        file_rows(&ctx).len(),
        3,
        "never more file rows than the card's three slots"
    );
    assert_eq!(
        diff_lines(&ctx).len(),
        8,
        "never more diff lines than the card's eight slots"
    );
}

/// The rows are data, not chrome: the body reads the CACHE, so a second preview
/// (the `diff.scope` cycle, `review.rs:483-491`) re-renders different paths.
#[test]
fn the_rows_follow_the_cache() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();
    let (_store, first) = ctx();
    assert_eq!(file_rows(&first)[0].0, "crates/app/src/main.rs");
    review::reset();
    review::fold_preview(
        &serde_json::from_str::<serde_json::Value>(
            r#"{"preview":{"files":[{"path":"other/x.rs","hunks":[{"header":"@@","lines":[
                {"kind":"added","content":"only","new_line":1}]}]}]}}"#,
        )
        .unwrap(),
    );
    let (_store2, second) = ctx();
    assert_eq!(
        file_rows(&second)[0].0, "other/x.rs",
        "a new preview must replace the rows, not accumulate"
    );
}

/// Machine-readable geometry beside the human doc (RULES: CSV with a header).
#[test]
fn write_the_row_geometry_row() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();
    let (_store, ctx) = ctx();
    let files = file_rows(&ctx);
    let lines = diff_lines(&ctx);
    // The `kind` value is written on EVERY row (not just the first of a group):
    // omitting it shifts that row's columns left and `csv.DictReader` then reads
    // the index into `kind` (found by parsing the emitted file, not by eye).
    let mut csv =
        String::from("kind,index,path_or_text,add,del,slot_count,bound\n");
    for (i, (path, add, del)) in files.iter().enumerate() {
        csv.push_str(&format!(
            "review_file_row,{i},{},{add},{del},3,bounded\n",
            path.replace(',', "_")
        ));
    }
    for (i, (text, num)) in lines.iter().enumerate() {
        csv.push_str(&format!(
            "diff_line,{i},{} {},,8,bounded\n",
            num.replace(',', "_"),
            text.replace(',', "_")
        ));
    }
    let path = format!(
        "{}/docs/review-sheet-rows.csv",
        env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/octoscode-module")
    );
    if let Some(dir) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&path, &csv).unwrap();
    println!("{csv}");
    assert!(!files.is_empty() && !lines.is_empty(), "rows must be non-empty");
}
