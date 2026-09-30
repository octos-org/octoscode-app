//! #32f — Android hardening tests.
//!
//! Item 2's contract: in the DARK theme the header title's ink equals the
//! theme's foreground token. The title is the shell's own Label
//! (`lib.rs` sidebar_header + first_run, `text: "OctosCode"`); its ink is the
//! DSL identifier `theme.color_fg_app`, which `role_assignments()` resolves
//! to the dark palette's foreground value (`#f5f5f7`; light pins `#1d1d1f`,
//! byte-identical to the pre-#31d shell). Both halves are pinned here: the
//! token VALUE per resolved theme, and the SOURCE binding of both title
//! labels to that same identifier — so the label can never silently fall
//! back to makepad's default (near-black) ink again (the phone bug).

use octoscode_module::screens::theme;

#[test]
fn the_title_ink_is_the_theme_foreground_token_in_dark() {
    // The token half: dark resolves the foreground role to the dark palette's
    // primary text value (the same one retint maps #1d1d1f ->).
    assert!(theme::set_preference("dark"), "dark is a valid preference");
    assert_eq!(theme::resolved(), "dark");
    let roles = theme::role_assignments();
    assert!(
        roles.contains("mod.theme.color_fg_app = #f5f5f7"),
        "dark foreground token missing: {roles}"
    );
    // The binding half: BOTH title labels reference that same token —
    // a label without an explicit ink falls back to makepad's default
    // (near-black), which is the dark-on-dark defect from the phone.
    let src = include_str!("../src/lib.rs");
    let segments: Vec<&str> = src.split("text: \"OctosCode\"").collect();
    assert_eq!(segments.len() - 1, 2, "both title labels pinned (sidebar + first_run)");
    for (i, seg) in segments.iter().skip(1).enumerate() {
        assert!(
            seg.contains("draw_text.color: theme.color_fg_app"),
            "title label #{i} is not bound to the foreground token"
        );
    }
    // Light keeps its own pinned value for the same token (the shell's
    // original literal, unchanged by this card).
    assert!(theme::set_preference("light"));
    let roles = theme::role_assignments();
    assert!(
        roles.contains("mod.theme.color_fg_app = #1d1d1f"),
        "light foreground token missing: {roles}"
    );
}
