//! Card #21d items 3 and 6 — two lowering post-processes in this module:
//!
//! * item 3: the person's bubble hugs, wraps and grows. The authored component
//!   fixes each label at `flow: Right` (no wrap) with a measured width, so a live
//!   message longer than the two-line fixture hard-clipped ("One wo…"). The
//!   lowering now lets the root hug up to 80% of the column and wraps the labels.
//! * item 6: icons load from the component's own files, not an asset server. The
//!   renderer wraps every SVG `src` in `http_resource(…)` naming the design lab's
//!   loopback server (`design.rs:743,764`); the lowering rewrites each to a
//!   `file_resource(…)` pointing at the bytes on disk beside the component.
//!
//! Both are pure string post-processes on the lowered DSL, so they are checked
//! here without a window.
use octoscode_module::components::{self, ItemKind};

fn copies(kind: ItemKind, text: &str) -> Vec<(String, String)> {
    components::resolve(kind)
        .0
        .bindings
        .iter()
        .map(|b| (b.copy.to_owned(), text.to_owned()))
        .collect()
}

#[test]
fn item3_the_bubble_hugs_wraps_and_is_capped() {
    let dsl = components::lower(ItemKind::UserBubble, "0", &copies(ItemKind::UserBubble, "hello"))
        .expect("the bubble lowers");
    assert!(
        dsl.contains("width: Fit max_width: \"80%\""),
        "the bubble root must hug and be capped at 80% of the column; got:\n{dsl}"
    );
    assert!(
        !dsl.contains("width: 284.01"),
        "the measured 284px box must not survive; got:\n{dsl}"
    );
    assert!(
        dsl.contains("flow: Right{wrap: true}"),
        "the bubble's labels must wrap, not hard-clip; got:\n{dsl}"
    );
    assert!(
        !dsl.contains("flow: Right\n"),
        "no unwrapped label may remain; got:\n{dsl}"
    );
    // Right-aligned in the column, like the approved scenes 01/03.
    assert!(
        dsl.starts_with("user_align := View{width:Fill height:Fit flow:Down align: Align{x: 1.0}"),
        "the bubble must stay right-aligned in the column; got:\n{dsl}"
    );
}

#[test]
fn item5_the_new_chat_row_carries_its_compose_icon() {
    let dsl = components::lower(ItemKind::NewChat, "0", &copies(ItemKind::NewChat, ""))
        .expect("new-chat lowers");
    assert!(
        dsl.contains("i0_newchat_icon := Svg"),
        "scene 01's new-chat row carries a compose icon; got:\n{dsl}"
    );
    assert!(
        dsl.contains("icon_compose.svg") && dsl.contains("file_resource(\"/"),
        "the compose icon binds the component's own file, not a URL; got:\n{dsl}"
    );
    assert!(
        !dsl.contains("http_resource("),
        "no icon may need a dev asset server; got:\n{dsl}"
    );
}

#[test]
fn item5_the_thread_title_ellipsizes() {
    let dsl = components::lower(ItemKind::ThreadRow, "0", &copies(ItemKind::ThreadRow, "x"))
        .expect("thread-row lowers");
    assert!(
        dsl.contains("max_lines: 1 text_overflow: TextOverflow.Ellipsis"),
        "the title must ellipsize, not hard-clip; got:\n{dsl}"
    );
    assert!(
        !dsl.contains("width: 302") && !dsl.contains("width: 379"),
        "the row must adopt the column's width, not keep the 406px panel's; got:\n{dsl}"
    );
}

#[test]
fn item6_icons_resolve_to_a_file_not_an_asset_server() {
    let dsl = components::lower(ItemKind::Composer, "0", &copies(ItemKind::Composer, ""))
        .expect("the composer lowers");
    assert!(
        !dsl.contains("http_resource("),
        "an icon must not need a dev asset server; got:\n{dsl}"
    );
    assert!(
        dsl.contains("file_resource(") && dsl.contains(".svg"),
        "icons must bind the component's own SVG file; got:\n{dsl}"
    );
    // The path points at the resolved component's assets on disk, not a URL.
    assert!(
        dsl.contains("assets/icon_send") && dsl.contains("file_resource(\"/"),
        "the icon file must be an absolute on-disk path; got:\n{dsl}"
    );
}
