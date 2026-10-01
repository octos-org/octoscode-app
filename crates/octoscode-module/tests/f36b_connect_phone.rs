//! Entry #36b — the Connect card at phone width (384x788, the 6T's logical
//! viewport), as a test on the PRODUCTION lowering path.
//!
//! Baseline measured on main with the instrument
//! (`MAKEPAD_REMOTE=8377 OCTOSCODE_SCREEN=connect OCTOSCODE_WINDOW_SIZE=384x788`,
//! `/snap?all=1`):
//!
//! | node                    | rect on main        |
//! |-------------------------|---------------------|
//! | card `beauty_0_0`       | [10, 32, 374, 734]  |
//! | server box              | [46, 214, 314, 44]  |
//! | server input            | [60, 226, 286, 26]  |  insets 12 / 6
//! | token box               | [46, 343, 314, 34]  |
//! | token input             | [60, 355, 286, 26]  |  bottom 381 > box 377
//!
//! So: the card's right edge is FLUSH (right margin 0), the token input hangs
//! 4px below its box, and the server input is 6px off-centre. The authored cause
//! is shared: both inputs are seated at a fixed +12 from the box top, but the
//! two boxes differ in height (44.3 vs 34.15) — see the card's `mapped.json`.
//!
//! The margins target is the WEB's phone rule:
//! `ConnectionPanel.module.css:319-321` —
//! `@media (max-width: 560px) { .gate { padding: 20px 16px } }` (equal sides).
//!
//! The card is authored on a fixed 406-wide artboard (`page w=406`,
//! `outer_card x=10 w=386` -> right edge 396), so at 384 it overflows by
//! construction; `connect::lower_screen` must therefore clamp it.

use std::collections::HashMap;

use octoscode_module::screens::connect::{self, ConnectUi, Screen};

const VIEWPORT_W: f64 = 384.0;
const VIEWPORT_H: f64 = 788.0;

/// The authored card geometry this test pins (design/stage-b/setup/cards/setup-01).
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn right(&self) -> f64 {
        self.x + self.w
    }
    fn bottom(&self) -> f64 {
        self.y + self.h
    }
}

/// Parse the lowered DSL's node rects.
///
/// The lowered head spans SEVERAL lines and is not always node-initializer
/// terminated (`beauty_0_0 := DesignSurface { width: 386 height: 756` runs on
/// to its children), so each node is scanned as its own indented block:
///
/// ```text
/// beauty_0_0 := DesignSurface {
/// width: 386 height: 756
/// abs_pos: vec2(10, 10)
/// ```
fn rects(dsl: &str) -> HashMap<String, Rect> {
    let mut out = HashMap::new();
    let num = |hay: &str, key: &str| -> Option<f64> {
        let at = hay.find(key)?;
        let tail = &hay[at + key.len()..];
        let end = tail
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(tail.len());
        tail[..end].parse().ok()
    };
    for (i, line) in dsl.lines().enumerate() {
        if !line.contains(" := ") {
            continue;
        }
        let name = line.split(" := ").next().unwrap_or("").trim().to_owned();
        if name.is_empty() {
            continue;
        }
        // the node's own head: its line plus following lines until a line that
        // starts a CHILD node (another `<name> := `) or closes the block
        let mut head = String::from(line);
        for l in dsl.lines().skip(i + 1) {
            if l.contains(" := ") || l.starts_with('}') {
                break;
            }
            head.push('\n');
            head.push_str(l);
        }
        let (Some(w), Some(h)) = (num(&head, "width: "), num(&head, "height: ")) else {
            continue;
        };
        let (x, y) = abs_pos(&head).unwrap_or((0.0, 0.0));
        out.insert(name, Rect { x, y, w, h });
    }
    out
}

/// `abs_pos: vec2(x, y)` -> (x, y) out of one node's head.
fn abs_pos(head: &str) -> Option<(f64, f64)> {
    let at = head.find("abs_pos: vec2(")?;
    let tail = &head[at + 14..];
    let close = tail.find(')')?;
    let mut it = tail[..close].split(',');
    let x = it.next()?.trim().parse().ok()?;
    let y = it.next()?.trim().parse().ok()?;
    Some((x, y))
}

/// One node's own lowered head, from `<node> := ` to the next node initializer
/// or closing brace — the same block `rects()` walks.
fn node_head<'a>(dsl: &'a str, node: &str) -> Option<&'a str> {
    let at = dsl.find(&format!("{node} := "))?;
    let rest = &dsl[at..];
    let end = rest[1..]
        .find("\n")
        .map(|i| at + 1 + i)
        .unwrap_or(dsl.len());
    let mut end = end;
    for line in dsl[end..].lines() {
        if line.contains(" := ") || line.starts_with('}') {
            break;
        }
        end += line.len() + 1;
    }
    Some(&dsl[at..end.min(dsl.len())])
}

fn lowered() -> String {
    connect::lower_screen(Screen::Connect, &ConnectUi::default()).expect("connect lowers")
}

/// The palette is PROCESS-GLOBAL (`theme::set_preference` / `theme::resolved`,
/// theme.rs:126/142), so any test that seeds or asserts the palette must not run
/// concurrently with another that does — without this the two dark tests
/// interleave across test threads and the run flakes (observed: 3 of 6 runs
/// failed before this lock).
fn palette_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// ITEM 1: the card must have EQUAL side margins inside the 384 viewport.
/// On main the card is 374 wide at x=10 -> right margin 0 (flush). Fails on main.
#[test]
fn the_connect_card_has_equal_side_margins_at_phone_width() {
    let dsl = lowered();
    let r = rects(&dsl);
    let card = r.get("beauty_0_0").expect("card node beauty_0_0");
    let left = card.x;
    let right = VIEWPORT_W - card.right();
    assert!(
        (left - right).abs() <= 1.0,
        "card x={} w={} -> left margin {left:.2}, right margin {right:.2} in a {VIEWPORT_W}px \
         viewport; the web gives equal sides (ConnectionPanel.module.css:319-321)",
        card.x,
        card.w
    );
    // and it must not overflow the viewport at all
    assert!(
        card.right() <= VIEWPORT_W + 1.0,
        "the card's right edge {} must stay inside the {VIEWPORT_W}px viewport \
         (the authored artboard is 406 wide)",
        card.right()
    );
}

/// ITEM 2: each TextInput must sit INSIDE its field box, vertically centred ±1px.
/// On main: server insets 12/6 (off-centre) and the token input's bottom (381)
/// is 4px BELOW its box bottom (377). Both fail on main.
#[test]
fn each_field_input_sits_centred_inside_its_box() {
    let dsl = lowered();
    let r = rects(&dsl);
    for (box_node, input_node, name) in [
        ("beauty_0_0_2", "beauty_0_0_2_0", "server"),
        ("beauty_0_0_4", "beauty_0_0_4_0", "token"),
    ] {
        let b = r
            .get(box_node)
            .unwrap_or_else(|| panic!("{name} box {box_node} in the lowered DSL"));
        let i = r
            .get(input_node)
            .unwrap_or_else(|| panic!("{name} input {input_node} in the lowered DSL"));
        let top = i.y - b.y;
        let bottom = b.bottom() - i.bottom();
        assert!(
            (top - bottom).abs() <= 1.0,
            "the {name} input is off-centre: top inset {top:.2}, bottom inset {bottom:.2} \
             (box {box_node} y={:.2} h={:.2}; input {input_node} y={:.2} h={:.2})",
            b.y,
            b.h,
            i.y,
            i.h
        );
        assert!(
            i.bottom() <= b.bottom() + 1.0,
            "the {name} input's bottom {:.2} must not fall below its box bottom {:.2}",
            i.bottom(),
            b.bottom()
        );
        assert!(
            i.top_inside(b),
            "the {name} input must sit fully inside its box"
        );
    }
}

impl Rect {
    fn top_inside(&self, b: &Rect) -> bool {
        self.y >= b.y - 1.0 && self.bottom() <= b.bottom() + 1.0
    }
}

/// The card must still fit the phone's HEIGHT (788) without clipping the
/// Connect button, which is the primary action.
#[test]
fn the_connect_button_stays_within_the_phone_viewport() {
    let dsl = lowered();
    let r = rects(&dsl);
    let btn = r.get("beauty_0_0_6").expect("connect button node");
    assert!(
        btn.bottom() <= VIEWPORT_H - 1.0,
        "the Connect button's bottom {:.2} must stay inside the {VIEWPORT_H}px phone viewport",
        btn.bottom()
    );
    assert!(
        btn.w > 0.0 && btn.h > 0.0,
        "the Connect button must keep a real hit area (got {}x{})",
        btn.w,
        btn.h
    );
}

/// ITEM 3: dark mode. The web's Connect is NOT light-only — `.gate`/`.card`
/// paint from design tokens (`var(--dsw-alias-bg-base)`,
/// `--dsw-alias-bg-layer-1`) and the feature has no `prefers-color-scheme` of
/// its own, so it follows the global theme. The app must therefore carry the
/// DARK palette on the Connect card, not the hardcoded light one.
///
/// There is no authored dark Connect twin on disk (`setup-01/kit/native/`
/// ships only `light`; `theme::card_for` has no connect entry), so #36b applies
/// the dark palette at the TOKEN level — the authored light hexes are remapped
/// to #31d's dark role values (`theme.rs:556-566`).
///
/// This asserts the shipped behaviour, and it is theme-sensitive: it checks
/// whichever palette is CURRENTLY resolved, so it must run with the palette
/// seeded. The dark seed is applied by the `dark_connect_palette` test below,
/// which is the one that fails on main.
#[test]
fn the_connect_card_carries_the_resolved_palette() {
    let _guard = palette_lock();
    let dsl = lowered();
    assert!(
        node_head(&dsl, "beauty_0_0").is_some(),
        "the card node must exist in the lowered DSL"
    );
    if octoscode_module::screens::theme::resolved() == "dark" {
        // dark: the #31d token values, and NO authored light surface left
        assert!(
            dsl.contains("draw_bg.color: #1c1f22ff"),
            "in dark the Connect surfaces must use the app's bg_app token #1c1f22"
        );
        assert!(
            dsl.contains("draw_bg.border_color: #38383aff"),
            "in dark the Connect card border must use the app's outset token #38383a"
        );
        assert!(
            !dsl.contains("draw_bg.color: #ffffffff"),
            "in dark no authored light surface (#ffffffff) may remain"
        );
    } else {
        // light: byte-identical to the authored Stage B output
        assert!(
            dsl.contains("draw_bg.color: #ffffffff"),
            "in light the Connect surfaces stay the authored #ffffff"
        );
        assert!(
            dsl.contains("draw_bg.border_color: #e5e5e7ff"),
            "in light the card border stays the authored #e5e5e7"
        );
    }
}

/// The dark half of item 3, as its own seedable test: with the palette forced
/// dark, the Connect DSL must carry the dark tokens. On main this fails — the
/// card stays light regardless of the theme (`OCTOSCODE_THEME` is read by
/// `theme::eval_roles`/`resolved`, but `connect::lower_screen` applied no
/// palette mapping, so the authored `#ffffffff` survives).
#[test]
fn dark_connect_palette() {
    // `resolved()` reads the process-global preference; seed it the way
    // lib.rs's mount arm does, then lower through the production path.
    let _guard = palette_lock();
    assert!(
        octoscode_module::screens::theme::set_preference("dark"),
        "the dark preference must be storable"
    );
    assert_eq!(octoscode_module::screens::theme::resolved(), "dark");
    let dsl = lowered();
    assert!(
        dsl.contains("draw_bg.color: #1c1f22ff"),
        "with the palette dark, the Connect card must paint the dark token \
         #1c1f22; on main it stayed the authored light #ffffffff"
    );
    assert!(
        !dsl.contains("draw_bg.color: #ffffffff"),
        "no authored light surface may survive in dark mode"
    );
    // restore light so the rest of the suite sees the authored palette
    octoscode_module::screens::theme::set_preference("light");
}

/// Machine-readable geometry beside the human doc (RULES: CSV with a header).
#[test]
fn write_the_geometry_row() {
    let dsl = lowered();
    let r = rects(&dsl);
    let card = r.get("beauty_0_0").expect("card");
    let left = card.x;
    let right = VIEWPORT_W - card.right();
    let mut row = format!(
        "node,label,x,y,w,h,right,bottom,left_margin,right_margin,centred\n\
         beauty_0_0,card,{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{left:.2},{right:.2},{}\n",
        card.x,
        card.y,
        card.w,
        card.h,
        card.right(),
        card.bottom(),
        (left - right).abs() <= 1.0
    );
    for (b, i, name) in [
        ("beauty_0_0_2", "beauty_0_0_2_0", "server"),
        ("beauty_0_0_4", "beauty_0_0_4_0", "token"),
    ] {
        let (bx, bx2) = (&r[b], &r[i]);
        let top = bx2.y - bx.y;
        let bottom = bx.bottom() - bx2.bottom();
        row.push_str(&format!(
            "{name},input,{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},,{:.2},{:.2},{}\n",
            bx2.x,
            bx2.y,
            bx2.w,
            bx2.h,
            bx2.right(),
            bx2.bottom(),
            top,
            bottom,
            (top - bottom).abs() <= 1.0
        ));
    }
    let path = format!(
        "{}/docs/connect-phone-geometry.csv",
        env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/octoscode-module")
    );
    if let Some(dir) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&path, &row).unwrap();
    println!("{row}");
}
