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
    // A1: the cap is the web's `min(680px, 82%)` of the LIVE column
    // (conv_layout), an absolute number the bubble hugs up to.
    let cap = octoscode_module::conv_layout::current().bubble_max_w;
    assert!(
        dsl.contains(&format!("RoundedView{{width: Fit height: Fit max_width: {cap}")),
        "the bubble must hug and be capped at min(680, 82%) of the column ({cap}); got:\n{dsl}"
    );
    assert!(
        !dsl.contains("width: 284.01") && !dsl.contains("width: 255.5"),
        "no measured artboard box may survive; got:\n{dsl}"
    );
    assert!(
        dsl.contains("flow: Right{wrap: true}"),
        "the bubble's label must wrap, not hard-clip; got:\n{dsl}"
    );
    // A Fit label wraps at its max only when line-limited (draw_text.rs:2245).
    assert!(
        dsl.contains("max_lines: "),
        "the Fit label must carry a line bound so its max width wraps; got:\n{dsl}"
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

/// Card #21h — the compose icon must sit at the card's right padding, not
/// overflow it.
///
/// #21c pinned the icon with an absolute `left = 220 − 24 − 4` (the old column
/// width). #21g then inset the card (207 wide), so the fixed left pushed the
/// icon's right edge to 216 > 207 and it was cut in half at the card edge
/// (live snap: `i0_newchat_icon r=[262,123,15,28]`). A `Fill`/`Align` wrapper is
/// not usable here (a `Fill` child under the KitButton root collapses to
/// `[0,0,0,0]`; `align` is the parent's property), so the wrapper keeps a fixed
/// size and its `left` must be computed from the card's real width. Assert the
/// geometry: `left + icon_width` must not exceed the card's 207px.
#[test]
fn item_h_the_compose_icon_fits_inside_the_card() {
    const CARD_W: f64 = 207.0; // threads_column 220 (lib.rs) − #21g's 13px inset
    let dsl = components::lower(ItemKind::NewChat, "0", &copies(ItemKind::NewChat, ""))
        .expect("new-chat lowers");
    // The icon wrapper's margin carries its `left`; parse it.
    let marker = "height: 28 margin: Inset{left: ";
    let at = dsl
        .find(marker)
        .unwrap_or_else(|| panic!("the icon wrapper must carry a left margin; got:\n{dsl}"));
    let rest = &dsl[at + marker.len()..];
    let left: f64 = rest[..rest.find(' ').unwrap()].parse().expect("a numeric left");
    let icon_w = 24.0;
    assert!(
        left + icon_w <= CARD_W,
        "the icon (left {left} + {icon_w}) overflows the {CARD_W}px card and would clip; got:\n{dsl}"
    );
    assert!(
        left + icon_w >= CARD_W - 8.0,
        "the icon must still sit at the card's right padding, not float left; got:\n{dsl}"
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

// ---- card #21f: three live-capture defects --------------------------------

/// Card #21f item 3 — the answer-actions timestamp must stay inside the content
/// width so the timeline's scrollbar cannot overlap it.
///
/// `ScrollBar { bar_size: 10, bar_side_margin: 3 }` (`scroll_bar.rs:25-27`) draws
/// a ~13px handle over the list's last pixels. The #21e revision anchored its
/// margin rewrite on `left: 268`, but the emitted wrapper carries the artboard's
/// own `left: 243.64` — so that replace never fired and the flush-right push put
/// `now` under the handle (`g4-interrupted.png`). Anchor on the real text and
/// leave a right inset wider than the handle.
#[test]
fn item3_the_timestamp_clears_the_scrollbar() {
    let dsl = components::lower(ItemKind::AnswerActions, "0", &copies(ItemKind::AnswerActions, "now"))
        .expect("the answer-actions lowers");
    assert!(
        !dsl.contains("left: 243.64"),
        "the artboard's absolute left must not survive into the mounted row; got:\n{dsl}"
    );
    // A1 (supersedes #21f's right-flush): on a 680 px column a right-flushed
    // time floated far from its answer (the operator's "detached" timestamp).
    // It now follows the action icons in the same row — and the list spans
    // the pane with the row centred inside ≥24 px gutters, so the scrollbar
    // (at the pane edge) can never sit on it.
    assert!(
        !dsl.contains("align: Align{x: 1.0"),
        "the timestamp must not be flushed to the column's far edge; got:\n{dsl}"
    );
    let icons = dsl.find("icon_share").expect("the share icon");
    let time = dsl.find("text: \"now\"").expect("the timestamp");
    assert!(icons < time, "the time follows the action icons; got:\n{dsl}");
}

/// Card #21g item 1 — the worked-for row is a small secondary-grey label over a
/// hairline rule.
///
/// The component's own kit tokens carry the size/weight/colour (≈0.85× the
/// prose body, weight 400, `#6b6b6b`); the lowering must also append the 1px
/// light rule the #16 ledger does not draw (`outer/codex-refs/03-worked.png`).
#[test]
fn item1_the_worked_for_row_is_small_grey_over_a_rule() {
    let dsl = components::lower(ItemKind::WorkedFor, "0", &copies(ItemKind::WorkedFor, "Worked for 3s ›"))
        .expect("the worked-for row lowers");
    // A1: the header of its turn's tool group (board 4 frame 1): the small
    // secondary size of the density (13 px desktop / 14 px phone = 9.75 /
    // 10.5 pt), the shell's muted ink, weight 400. The Codex rule under the
    // row is gone — the row now heads its tool card instead of trailing it.
    let small_pt = octoscode_module::fluid::scale(octoscode_module::conv_layout::current().density).small * 0.75;
    assert!(
        dsl.contains(&format!("font_size: {small_pt} ")),
        "the label must use the small size ({small_pt}pt); got:\n{dsl}"
    );
    assert!(
        dsl.contains("draw_text.color: #6e6e73ff") || dsl.contains("draw_text.color: #98989dff"),
        "the label must be the shell's secondary ink; got:\n{dsl}"
    );
    assert!(
        dsl.contains("weight: 400"),
        "the label must be weight 400, not 500; got:\n{dsl}"
    );
    assert!(
        dsl.contains("text: \"Worked for 3s\""),
        "the words carry no `›` glyph (the chevron is an icon); got:\n{dsl}"
    );
}

/// Card #21g item 2 — a code block must not HARD-CLIP an over-long line.
///
/// The #21e pin ran `code_layout` non-wrapping, so a long line was cut at the
/// block's right edge (`g4-interrupted.png`: `let y = x + 10; // panic in
/// debug,…`). The board's accepted alternative to horizontal scroll is a
/// visible soft-wrap, so the theme's wrapping code layout must survive.
#[test]
fn item2_a_long_code_line_is_reachable_not_clipped() {
    // The board's fixture ("let y = x + 10; // panic in debug,…") widened past
    // 120 columns.
    let line = format!("let y = x + 10; // {}", "panic in debug ".repeat(10));
    assert!(
        line.chars().count() > 120,
        "the fixture line must exceed 120 columns"
    );
    let dsl = components::lower(
        ItemKind::AssistantProse,
        "0",
        &copies(ItemKind::AssistantProse, &line),
    )
    .expect("the prose lowers");
    assert!(
        !dsl.contains("code_layout: Layout{flow: Right}"),
        "the non-wrapping #21e pin must be gone (it hard-clips long lines); got:\n{dsl}"
    );
    assert!(
        dsl.contains(&line),
        "the full >120-column line must survive into the markdown body; got:\n{dsl}"
    );
}

/// Card #21f item 2 — the cleared second bubble line must not reserve a line box.
///
/// The component's `t02` is bound to `@clear` (the whole message rides the
/// wrapping `t01`), but an empty `Label` still measured one line (~29px), which
/// read as a dead black band under the text (`g3-completed.png`).
#[test]
fn item2_the_cleared_bubble_line_does_not_reserve_height() {
    let dsl = components::lower(ItemKind::UserBubble, "0", &copies(ItemKind::UserBubble, "hi"))
        .expect("the bubble lowers");
    // A1: the whole message rides ONE wrapping label; the artboard's second
    // fixture line no longer exists, so it cannot reserve a line box.
    assert_eq!(
        dsl.matches(":= Label{").count(),
        1,
        "the bubble must carry exactly one text label; got:\n{dsl}"
    );
    assert!(!dsl.contains("i0_userbubble_1"), "no second fixture label; got:\n{dsl}");
}
