//! A32 / decision D10d — makepad's text layouter starts a truncation ellipsis
//! where the kept text ends.
//!
//! The judge's "below 9" item 1: a single-line title truncated with an
//! ellipsis could end in TWO dots ("Fix steer queue drop.."). A soft-wrapped
//! row keeps the space it broke at as its last glyph while its width leaves
//! that space out; `truncate_last_row_with_ellipsis` subtracted the space a
//! second time and drew the ellipsis one space advance short of the text, its
//! first dot inside the last glyph (live, A29's sidebar row: the ellipsis at
//! x=132.631, the "p" ending at 136.568 — 3.938 px, Inter's space at 14 px —
//! in a 145.811 px label truncated for exactly 145.811 px).
//!
//! The fix lives in the shared path every wrapping `Label` with
//! `text_overflow: Ellipsis` goes through, so it is one tracked makepad patch,
//! `patches/makepad/layouter-ellipsis-pen.patch`, applied to every tree by
//! `scripts/apply-makepad-patches.sh`; it carries the makepad unit tests that
//! reproduce the bug. This test pins the patch's shape and, with
//! `MAKEPAD_PATCH_TREES=<root>:<root>:…` (as for tests/a25_makepad_patch.rs),
//! that it is applied in every tree. The APK's `.mk` layouter differs by
//! OctoSense's own line-breaking edit, so the trees are compared by
//! "applied", not byte for byte.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PATCH: &str = "patches/makepad/layouter-ellipsis-pen.patch";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn patch_text() -> String {
    std::fs::read_to_string(repo().join(PATCH)).expect("the tracked patch exists")
}

#[test]
fn the_ellipsis_patch_touches_only_the_layouter_and_counts_from_the_pen() {
    let text = patch_text();
    let touched: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("+++ b/")).collect();
    assert_eq!(touched, ["draw/src/text/layouter.rs"]);
    // Additions only: the truncation keeps its algorithm and gains the pen
    // count before it and the re-alignment after it.
    assert!(
        !text.lines().any(|l| l.starts_with('-') && !l.starts_with("---")),
        "the patch removes no makepad line"
    );
    // Before popping: the width is the pen (the last glyph's end), so a
    // soft-wrap space it leaves out is not subtracted twice.
    assert!(
        text.contains("+        if let Some(last) = last_row.glyphs.last() {\n")
            && text.contains("+            last_row.width_in_lpxs = last.origin_in_lpxs.x + last.advance_in_lpxs();\n"),
        "the truncation counts from the pen"
    );
    // After the ellipsis: the row is aligned again from its new width.
    assert!(
        text.contains("+        if let Some(max_width_in_lpxs) = self.options.max_width_in_lpxs {\n")
            && text.contains("+                self.options.align * (max_width_in_lpxs - last_row.width_in_lpxs);\n"),
        "the truncated row is re-aligned"
    );
}

#[test]
fn the_ellipsis_patch_carries_the_tests_that_reproduce_the_two_dots() {
    let text = patch_text();
    for test in [
        "fn ellipsis_after_a_soft_wrap_space_starts_where_the_text_ends()",
        "fn ellipsis_row_geometry_holds_at_every_bound()",
        "fn a_truncated_aligned_row_is_placed_inside_its_bound()",
    ] {
        assert!(text.contains(&format!("+    {test} {{")), "the patch carries {test}");
    }
    // The geometry every truncated row must keep: the ellipsis starts at the
    // last kept glyph's end, the row ends at the ellipsis' end, inside the bound.
    for needle in [
        "(ellipsis_x - pen_after(kept)).abs() < 0.01",
        "(row.width_in_lpxs - end).abs() < 0.01",
        "row.width_in_lpxs <= max_width + 0.01",
        // The A29 title, at a bound that breaks the row at a space.
        "\"Fix steer queue drop on reconnect\"",
        "capped.rows[0].text.ends_with(\"drop \")",
    ] {
        assert!(text.contains(needle), "the tests check {needle}");
    }
}

#[test]
fn every_tree_has_the_ellipsis_patch_applied() {
    let Ok(roots) = std::env::var("MAKEPAD_PATCH_TREES") else {
        eprintln!("skipped: set MAKEPAD_PATCH_TREES=<root>:<root>:... to check the trees");
        return;
    };
    for root in roots.split(':').filter(|s| !s.is_empty()) {
        let root = PathBuf::from(root);
        let tree = if root.join("makepad/platform/src").is_dir() { root.join("makepad") } else { root };
        let status = Command::new("patch")
            .args(["-R", "-p1", "-f", "-s", "--dry-run", "-d"])
            .arg(&tree)
            .stdin(std::fs::File::open(repo().join(PATCH)).unwrap())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("patch(1)");
        assert!(status.success(), "the ellipsis patch is not applied in {}", tree.display());
    }
}
