//! Entry #32b2 — conversation timeline polish (#32b items 1/5/8):
//! 1. the user bubble hugs its content (no empty band under a 1-line question,
//!    side-question screen included);
//! 2. the timeline scrollbar must not draw over the user bubble's right edge;
//! 3. the attachments "68%" label must not overlap the progress ring.

use std::sync::Arc;

use octoscode_store::Store;

// ------------------------------------------------------------- anchors dump

#[test]
fn dump_the_aside_and_attachments_lowered_dsl() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    octoscode_module::screens::sessions::seed_aside("How do I stream tokens?", "Use the stream API.");
    let aside = octoscode_module::screens::sessions::lower_screen("aside", &store).unwrap();
    dump_context(&aside, &["user_bubble", "t_q1"], "ASIDE");
    // The attachments arm cuts att2 entirely at 0 items — seed two so the
    // ring/pct nodes exist to inspect.
    octoscode_module::screens::sessions::seed_attachments(vec![("a.png", 1024), ("b.png", 2048)]);
    let att = octoscode_module::screens::sessions::lower_screen("attachments", &store).unwrap();
    dump_context(&att, &["att2_ring", "att2_pct"], "ATT");
}

/// Print each anchor line PLUS the following lines (the node's interior:
/// width/height/margin carry the geometry the fixes must patch).
fn dump_context(dsl: &str, anchors: &[&str], tag: &str) {
    let lines: Vec<&str> = dsl.lines().collect();
    let mut skip = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if skip > 0 {
            eprintln!("{tag}+| {line}");
            skip -= 1;
            continue;
        }
        if anchors.iter().any(|a| line.contains(a)) {
            eprintln!("{tag} | {line}");
            skip = 14;
        }
    }
}

/// Item 1: the aside bubble surface hugs its one-line question — the authored
/// 90px box (width: 289 height: 90) left an empty band under the text.
#[test]
fn aside_user_bubble_hugs_its_content() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    octoscode_module::screens::sessions::seed_aside("How?", "Answer.");
    let aside = octoscode_module::screens::sessions::lower_screen("aside", &store).unwrap();
    assert!(
        aside.contains("user_bubble := DesignSurface {\nwidth: 289 height: Fit"),
        "the bubble surface must hug its content (was the authored 90)"
    );
    assert!(!aside.contains("height: 90"), "no fixed 90 may remain");
}
