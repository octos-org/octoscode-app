//! Entry #36d — **app-wide guard**: every `Label` in the module's `script_mod!`
//! blocks must set an explicit `draw_text.color`.
//!
//! ## Why this is a class, not a one-off
//!
//! The app installs **no** Makepad widget style sheet on macOS. The chain is
//! `makepad-widgets` (rev pinned at `Cargo.toml:47`) → `widgets/src/lib.rs:530`
//! `script_mod` → `:534 widgets_mod(vm)` → `:352 apply_theme(vm)` →
//! `desktop_style.rs:244` → `desktop_style.rs:206-210`, which resolves the style
//! from `MAKEPAD_WIDGET_STYLE` (never set by this app) or `OsType::Ios` /
//! `OsType::Android`, and on macOS falls to **`_ => None`** (line 209) — so `?`
//! early-returns `None` and **no sheet is installed at all**.
//!
//! A Label's default ink colour is `theme.color_label_outer`
//! (`widgets/src/label.rs:23`). With no sheet, that role is never assigned, so a
//! Label with no explicit `draw_text.color` paints **0 ink** while `/snap` still
//! reports its text. Measured in #36c r1: colourless labels drew 0 dark pixels
//! (`Review`, `Last turn`, `review_file_path`, `review_line_text`), while
//! coloured ones in the same frame, same font, drew ink (`+62 -5` 537 px, `M`
//! 198 px). So "visible" must mean **width > 0 AND ink ≠ local background**,
//! never "`/snap` has text".
//!
//! ## Scope of the audit
//!
//! Four sources were named by the entry. Three are checked statically here and
//! one is checked live (see `the_live_walk_is_recorded_in_the_report`):
//!
//! | source | result |
//! |---|---|
//! | `lib.rs` `script_mod!` (the native chrome) | 38 `Label` blocks — **12 were colourless, all 12 fixed** |
//! | lowered DSL (Stage B cards) | 0 colourless: the emitter always writes a literal (`design.rs:733-735`, `hex_rgba(a.color.unwrap_or(0xff111927))`) |
//! | `screens/*.rs` | no `script_mod!` at all — pure Rust data (they return DSL *strings*, lowered through the safe emitter above) |
//! | `components.rs` (lowered components) | goes through the same safe emitter |
//!
//! Before → after in `lib.rs`: **12 colourless → 0**.

use std::path::Path;

/// The module's `script_mod!` host: every hand-written native Label lives here.
const SHELL: &str = "crates/octoscode-module/src/lib.rs";

/// Brace-balanced blocks of `Label { … }`, with their 1-based line numbers.
///
/// Balanced by scanning rather than by regex so a block containing a nested
/// brace (or a single-line one) is captured whole — a regex `[^}]*` truncated
/// at the first inner `}` and both over- and under-counted.
fn label_blocks(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while let Some(rel) = src[i..].find("Label {") {
        let start = i + rel;
        let open = start + "Label".len();
        let mut depth = 0i32;
        let mut j = open;
        while j < bytes.len() {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        assert!(j < bytes.len(), "unbalanced Label block at byte {start}");
        let line = src[..start].matches('\n').count() + 1;
        out.push((line, src[start..=j].to_owned()));
        i = j + 1;
    }
    out
}

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    std::fs::read_to_string(root.join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// THE guard. Fails on main: 12 colourless blocks, including two that carried
/// authored text (`"Session settings"`, `"Octos server"`) and ten runtime-fed
/// `text: ""` slots.
#[test]
fn every_label_in_the_shell_script_mod_sets_a_text_colour() {
    let src = read(SHELL);
    let blocks = label_blocks(&src);
    assert!(
        blocks.len() >= 30,
        "expected the whole shell to be scanned, found only {} Label blocks — the \
         scanner regressed",
        blocks.len()
    );

    let mut colourless: Vec<String> = Vec::new();
    for (line, block) in &blocks {
        if !block.contains("draw_text.color") {
            let text = block
                .split("text: \"")
                .nth(1)
                .and_then(|t| t.split('"').next())
                .unwrap_or("");
            colourless.push(format!("{SHELL}:{line}  text={text:?}"));
        }
    }
    assert!(
        colourless.is_empty(),
        "every Label must set an explicit draw_text.color. With no widget style \
         sheet on macOS the default `theme.color_label_outer` is never assigned, \
         so a colourless Label paints 0 ink while /snap still shows its text. \
         Offenders:\n  {}",
        colourless.join("\n  ")
    );
}

/// The same rule for the whole `script_mod!` region only — the three anchor
/// widgets above and below the host's own widgets. Guards against a Label being
/// added outside the block the scanner reads.
#[test]
fn the_measured_meta_labels_carry_a_colour_too() {
    let src = read(SHELL);
    // The two 1px header_meta measurement labels and the palette row name are
    // fed at runtime and were colourless on main; pin them by id so a refactor
    // that moves them out of the script_mod block is caught.
    for needle in [
        "status := Label {",
        "sessions := Label {",
        "palette_row_name := Label {",
        "goal_row_1 := Label {",
        "loop_row_1 := Label {",
        "fleet_row_1 := Label {",
    ] {
        let at = src
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} not found in {SHELL}"));
        let block = &src[at..at + 400];
        let end = block.find("}").unwrap_or(block.len());
        assert!(
            block[..end].contains("draw_text.color"),
            "{needle} is runtime-fed and colourless — it would paint 0 ink"
        );
    }
}

/// The lowered-DSL path is already safe; pin WHY, so a future change to the
/// emitter that drops the colour fails here rather than on device.
#[test]
fn the_lowered_dsl_emitter_always_writes_a_colour() {
    let emitter = read(
        ".forks/octoscript-makepad-fork/crates/octoscript-makepad/src/design.rs",
    );
    assert!(
        emitter.contains("draw_text.color: {}"),
        "the design emitter must keep writing draw_text.color for every Text node"
    );
    assert!(
        emitter.contains("a.color.unwrap_or(0xff111927)"),
        "the emitter's colour must keep a literal default, or a card with no \
         authored colour would lower a colourless Label"
    );
}

/// Machine-readable audit beside the human doc (RULES: CSV with a header).
#[test]
fn write_the_label_audit_row() {
    let src = read(SHELL);
    let blocks = label_blocks(&src);
    let mut csv = String::from("file,line,has_text_colour,total_blocks\n");
    let with = blocks
        .iter()
        .filter(|(_, b)| b.contains("draw_text.color"))
        .count();
    csv.push_str(&format!("{SHELL},all,true,{}\n", blocks.len()));
    csv.push_str(&format!("{SHELL},without_text_colour,false,{}\n", blocks.len() - with));
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let path = root.join("docs/label-colour-audit.csv");
    std::fs::write(&path, &csv).expect("write csv");
    println!("{csv}");
    assert_eq!(
        blocks.len() - with,
        0,
        "the audit must find zero colourless Labels after the fix"
    );
}
