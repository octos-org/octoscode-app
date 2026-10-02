//! Entry #36e item 2 — the web's **+/− diff line tint**, baked as **literal
//! hexes chosen at lower time** (no new `theme.*` role).
//!
//! ## The oracle
//!
//! `DiffReviewDialog.module.css:45-54`:
//!
//! ```css
//! :global(.diff-added)   .changedWord { background: color-mix(in srgb, var(--dsw-alias-state-success-primary) 22%, transparent); }
//! :global(.diff-removed) .changedWord { background: color-mix(in srgb, var(--dsw-alias-state-error-primary)   20%, transparent); }
//! ```
//!
//! Tokens at `app/theme.css:86-89` (`success #22c55e`; `error
//! light-dark(#ec1313, #ff6b6b)`). A **context** line gets no fill
//! (`diff-line`, `:35-39`, sets only font-size/colour). A `color-mix` over a
//! surface resolves per palette, so the pre-mixed pair differs light vs dark.
//!
//! ## Why LITERALS and not a role (the constraint the outer loop set)
//!
//! Assigning a NEW `theme.*` role in `role_assignments` is not readable by a
//! widget default on this host. The module's `script_mod` then aborts with
//!
//! ```text
//! [E] lib.rs:715       property color_diff_del not found in prototype chain
//! [E] keys_probe.rs:330  variable OctoscodeView not found in scope
//! ```
//!
//! so `OctoscodeView` is never registered and **the whole app fails to mount**.
//! A/B against the base isolates it: substituting the EXISTING
//! `theme.color_bg_odd` gave 0 `[E]` lines and a mounted shell; a rebuilt clean
//! base gave 0 `[E]` / 250 entries. Existing roles resolve, a new one does not.
//! So the tint is emitted as a literal at lower time — the same shape the
//! design emitter uses when it writes `hex_rgba` literals
//! (`octoscript-makepad/src/design.rs:733-735`), spliced in with `#(...)`
//! (`lib.rs:121` is the precedent), so the script never resolves a role.
//!
//! ## What the test can and cannot see (a gap I hit, stated up front)
//!
//! My first attempt at this test was **4/4 green while the app did not mount**,
//! because every assert inspected SOURCE SHAPE. So the assertions here include
//! the exact strings the script must contain — the literals, the layers, the
//! `visible: false` defaults, the mark-driven `set_visible` — AND a guard that
//! the tint is NOT expressed as a role, which is the specific regression that
//! broke the app. A runtime-only failure would still need a live walk; the
//! light/dark walk that backs this lives in .peer/report-36e.md.
//!
//! ## Fails on main
//!
//! On main there is no `diff_tint_hexes`, no tint layer, and every diff row
//! paints the panel background — `+`, `−` and context are indistinguishable.

use octoscode_module::screens::review::{self, Line};
use octoscode_module::screens::theme;

fn seed_preview() {
    // The recorded r5-turn `diff/preview/get` shape (the fixture f30a and f36c
    // fold): one file whose first hunk carries a context, a removed and two
    // added lines, so all three tints appear in a single file.
    let v: serde_json::Value = serde_json::from_str(
        r#"{"preview":{"session_id":"dsflash:main","preview_id":"01920000-0000-7000-8000-0000000000f1",
        "title":"Working tree",
        "files":[{"path":"crates/app/src/main.rs","status":"modified","hunks":[{"header":"@@",
          "lines":[
            {"kind":"context","content":"fn main() {","old_line":1,"new_line":1},
            {"kind":"removed","content":"    run(old);","old_line":2},
            {"kind":"added","content":"    run(new);","new_line":2},
            {"kind":"added","content":"    check();","new_line":3}
          ]}]}]}}"#,
    )
    .expect("the recorded preview fixture must parse");
    review::fold_preview(&v);
}

/// The mark each of the eight bound diff rows resolves to, in slot order.
fn row_marks() -> Vec<&'static str> {
    let st = review::ui();
    st.lines.iter().take(8).map(Line::mark).collect()
}

/// With the recorded preview folded, the rows must resolve to context, removed,
/// added, added — i.e. THREE distinguishable classes, so `+ != - != context` is
/// decidable at all.
#[test]
fn a_seeded_preview_yields_three_distinguishable_row_kinds() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();

    let marks = row_marks();
    assert_eq!(
        marks,
        vec!["", "-", "+", "+"],
        "the folded lines are context, removed, added, added (the web's diffKind \
         collapse, diff-presentation.ts:17-23)"
    );
    assert_eq!(marks.iter().filter(|m| **m == "+").count(), 2);
    assert_eq!(marks.iter().filter(|m| **m == "-").count(), 1);
    assert_eq!(marks.iter().filter(|m| m.is_empty()).count(), 1);
}

/// The literal pair must differ BETWEEN PALETTES — the web's `color-mix`
/// composites over a different surface in each. A single pair would mean the
/// tint is not palette-aware, which the entry's "in light and dark" forbids.
#[test]
fn the_literal_tint_hexes_differ_between_light_and_dark() {
    let (light_add, light_del) = hexes_for("light");
    let (dark_add, dark_del) = hexes_for("dark");

    assert_ne!(
        light_add, dark_add,
        "the ADDED fill must differ per palette (light {light_add}, dark {dark_add})"
    );
    assert_ne!(
        light_del, dark_del,
        "the REMOVED fill must differ per palette (light {light_del}, dark {dark_del})"
    );
    // And within a palette the two kinds must not collide — otherwise `+` and
    // `-` are indistinguishable, which is the whole point of the card.
    assert_ne!(light_add, light_del, "light: + fill must differ from - fill");
    assert_ne!(dark_add, dark_del, "dark: + fill must differ from - fill");

    // Every value is a 6-digit hex: a literal, not a role reference.
    for (branch, a, d) in [("light", light_add, light_del), ("dark", dark_add, dark_del)] {
        for (kind, v) in [("added", a), ("removed", d)] {
            assert!(
                v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "{branch}/{kind} must be a literal #rrggbb, got {v:?}"
            );
        }
    }
}

/// `diff_tint_hexes()` is what the lowering calls, and it must return the
/// branch's pair. This pins the helper to the palette `resolved()` reports, so a
/// `match` that ignores the branch cannot pass.
/// A13: the tests here that flip the process-wide theme preference run one at
/// a time (`theme::test_lock` is `#[cfg(test)]`, out of reach of this binary):
/// on 4 test threads `the_helper_agrees_with_the_hardcoded_layers` read the
/// light pair right after setting dark (the full suite failed once that way).
fn theme_flip_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

#[test]
fn the_helper_follows_the_resolved_palette() {
    let _flip = theme_flip_lock();
    for want in ["light", "dark"] {
        theme::set_preference(want);
        let expected = if want == "light" {
            hexes_for("light")
        } else {
            hexes_for("dark")
        };
        let got = theme::diff_tint_hexes();
        assert_eq!(
            (got.0.to_string(), got.1.to_string()),
            (expected.0.to_string(), expected.1.to_string()),
            "with OCTOSCODE_THEME-equivalent {want}, diff_tint_hexes() must return \
             that branch's pair"
        );
        // A context row paints NO fill, so the helper exposes only the two
        // tints — there is deliberately no third value.
        assert!(
            !got.0.is_empty() && !got.1.is_empty() && got.0 != got.1,
            "{want}: the two tints must be present and distinct"
        );
    }
    theme::set_preference("light");
}

/// The regression that broke the app: the tint must NOT be a `theme.*` role, and
/// must NOT be a `#(...)` splice either. Both were measured failures:
///
/// - a new role → `property … not found in prototype chain` → the module's
///   `script_mod` aborts → `OctoscodeView` unregistered → the app mounts nothing;
/// - a spliced `draw_bg.color` → delivers NOTHING. The A/B in one run: a
///   hardcoded `#00ff00` layer painted `(0,255,0)` while the spliced layer beside
///   it stayed the panel colour; a control layer with a hardcoded `#ff00ff`
///   painted too, and a saturated `#00ff00` through the splice still did not.
///
/// So the four layers are HARD-CODED literals — two per palette.
#[test]
fn the_four_tint_layers_are_hardcoded_literals() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lib.rs");
    let theme_src =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/screens/theme.rs"))
            .expect("read theme.rs");

    assert!(
        !theme_src.contains("mod.theme.color_diff"),
        "a new `mod.theme.color_diff*` role must NOT be assigned: an unreadable \
         role aborts script_mod (`property … not found in prototype chain` -> \
         `OctoscodeView not found`) and the app mounts nothing"
    );
    assert!(
        !src.contains("diff_tint_hexes()"),
        "the template must not read a `#(...)` splice for its colour — measured \
         to deliver no colour at all (a hardcoded literal in the same row paints)"
    );

    for (layer, hex) in [
        ("review_line_bg_add_light", "#cef2dc"),
        ("review_line_bg_del_light", "#fbd4d4"),
        ("review_line_bg_add_dark", "#1d442f"),
        ("review_line_bg_del_dark", "#462425"),
    ] {
        let at = src
            .find(&format!("{layer} := SolidView {{"))
            .unwrap_or_else(|| panic!("{layer} in the shell"));
        let body = &src[at..];
        let end = brace_end(body);
        let block = &body[..end];
        assert!(
            block.contains(&format!("draw_bg.color: {hex}")),
            "{layer} must carry the literal {hex} (the web's color-mix, \
             pre-mixed against that palette's row surface)"
        );
        assert!(
            !block.contains("theme.color_diff"),
            "{layer} must not name a theme role (that is the aborting shape)"
        );
        assert!(
            block.contains("visible: false"),
            "{layer} must start hidden — a context row must paint no fill"
        );
    }
}

/// The four literals must be three-way distinct per palette, and the two
/// palettes must differ — `+` ≠ `−` ≠ context is the entry's proof bar, and it
/// is only decidable if the fills actually differ.
#[test]
fn the_literals_are_three_way_distinct_per_palette() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lib.rs");
    let hex = |layer: &str| -> String {
        let at = src
            .find(&format!("{layer} := SolidView {{"))
            .unwrap_or_else(|| panic!("{layer} in the shell"));
        let body = &src[at..];
        let end = brace_end(body);
        body[..end]
            .split("draw_bg.color: #")
            .nth(1)
            .map(|t| t.chars().take(6).collect::<String>())
            .expect("a literal hex")
    };
    let (la, ld) = (hex("review_line_bg_add_light"), hex("review_line_bg_del_light"));
    let (da, dd) = (hex("review_line_bg_add_dark"), hex("review_line_bg_del_dark"));
    // A context row paints nothing, so the row surface is the panel colour —
    // distinct from both tints by construction. Within a palette, + and − must
    // differ, else the tint says nothing.
    assert_ne!(la, ld, "light: the + and − fills must differ");
    assert_ne!(da, dd, "dark: the + and − fills must differ");
    // And the palettes must differ, or "in light and dark" is unmet.
    assert_ne!(la, da, "the + fill must differ between palettes");
    assert_ne!(ld, dd, "the − fill must differ between palettes");
    for (branch, a, d) in [("light", &la, &ld), ("dark", &da, &dd)] {
        for (kind, v) in [("added", a), ("removed", d)] {
            assert_eq!(
                v.len(),
                6,
                "{branch}/{kind}: {v:?} must be 6 hex digits"
            );
        }
    }
}

/// The feed drives the palette's pair, off `Line::mark`.
#[test]
fn the_shell_drives_the_palette_pair_from_the_line_mark() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lib.rs");

    assert!(
        src.contains("Line::mark"),
        "the feed must read Line::mark — review.markN exists only for N=2..6 and \
         there is no review.kindN, so a binding-driven tint would tint the \
         wrong rows"
    );
    // All four ids are reachable from the feed.
    for layer in [
        "review_line_bg_add_light",
        "review_line_bg_del_light",
        "review_line_bg_add_dark",
        "review_line_bg_del_dark",
    ] {
        assert!(
            src.contains(&format!("live_id!({layer})")),
            "the feed must be able to show {layer}"
        );
    }
    // The palette is selected at feed time, not at author time.
    assert!(
        src.contains("theme::resolved() == \"dark\""),
        "the feed must pick the palette's pair from theme::resolved() — that is \
         what makes one authored template serve both palettes"
    );
    // And each kind is gated on its mark.
    for mark in ["\"+\"", "\"-\""] {
        assert!(
            src.contains(&format!("*mark == {mark}")),
            "a layer must be shown when the line's mark is {mark}"
        );
    }
}

/// **Drift guard.** `theme::diff_tint_hexes()` is no longer what the template
/// reads — the splice delivers nothing (measured), so the four layers carry
/// hardcoded literals. The helper stays as the *statement of the mix math*, which
/// makes it a second source of truth for the same four values. Pin them together
/// so editing one without the other fails here rather than silently drifting.
#[test]
fn the_helper_agrees_with_the_hardcoded_layers() {
    let _flip = theme_flip_lock();
    theme::set_preference("light");
    let (la, ld) = theme::diff_tint_hexes();
    theme::set_preference("dark");
    let (da, dd) = theme::diff_tint_hexes();
    theme::set_preference("light");

    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lib.rs");
    let literal = |layer: &str| -> String {
        let at = src
            .find(&format!("{layer} := SolidView {{"))
            .unwrap_or_else(|| panic!("{layer} in the shell"));
        let body = &src[at..];
        let end = brace_end(body);
        body[..end]
            .split("draw_bg.color: #")
            .nth(1)
            .map(|t| t.chars().take(6).collect::<String>())
            .expect("a literal hex")
    };

    assert_eq!(
        literal("review_line_bg_add_light"),
        la.trim_start_matches('#'),
        "the helper's light ADD and review_line_bg_add_light disagree"
    );
    assert_eq!(
        literal("review_line_bg_del_light"),
        ld.trim_start_matches('#'),
        "the helper's light DEL and review_line_bg_del_light disagree"
    );
    assert_eq!(
        literal("review_line_bg_add_dark"),
        da.trim_start_matches('#'),
        "the helper's dark ADD and review_line_bg_add_dark disagree"
    );
    assert_eq!(
        literal("review_line_bg_del_dark"),
        dd.trim_start_matches('#'),
        "the helper's dark DEL and review_line_bg_del_dark disagree"
    );
}

/// Machine-readable row beside the human doc (RULES: CSV with a header).
#[test]
fn write_the_tint_row() {
    let _seq = review::test_lock();
    review::reset();
    seed_preview();
    let marks = row_marks();
    let (la, ld) = hexes_for("light");
    let (da, dd) = hexes_for("dark");

    let mut csv = String::from("slot,mark,line_kind,tint_layer,light_fill,dark_fill\n");
    let lines: Vec<(String, String)> = {
        let st = review::ui();
        st.lines.iter().take(8).map(|l| (l.kind.clone(), l.content.clone())).collect()
    };
    for (i, mark) in marks.iter().enumerate() {
        let (layer, l, d): (&str, &str, &str) = match *mark {
            "+" => ("review_line_bg_add", la.as_str(), da.as_str()),
            "-" => ("review_line_bg_del", ld.as_str(), dd.as_str()),
            _ => ("none", "", ""),
        };
        let (kind, content) = lines
            .get(i)
            .map(|(k, c)| (k.as_str(), c.as_str()))
            .unwrap_or(("", ""));
        csv.push_str(&format!(
            "{i},{mark},{kind},{layer},{},{}\n",
            l.trim_start_matches('#'),
            d.trim_start_matches('#')
        ));
        let _ = content;
    }
    let path = format!(
        "{}/docs/review-diff-tint.csv",
        env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/octoscode-module")
    );
    if let Some(dir) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&path, &csv).expect("write csv");
    println!("{csv}");

    assert!(
        marks.iter().any(|m| *m == "+") && marks.iter().any(|m| *m == "-"),
        "the fixture must contain both an added and a removed line"
    );
    assert_ne!(la, ld);
    assert_ne!(da, dd);
}

/// `brace_end`: index of the brace-balanced end of `body` (which starts at a
/// `{`). A plain `find("}")` stops at an inner `Inset{…}`.
fn brace_end(body: &str) -> usize {
    let mut depth = 0i32;
    for (i, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    body.len()
}

/// `(added, removed)` literal hexes for a branch, read from the source so the
/// assertion tracks the implementation instead of restating it.
fn hexes_for(branch: &str) -> (String, String) {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/screens/theme.rs"
    ))
    .expect("read theme.rs");
    // The light arm is the `"light" => (… )` line, the dark arm the `_ => (… )`.
    let arm = if branch == "light" {
        let at = src
            .find("\"light\" => (")
            .unwrap_or_else(|| panic!("a light arm in diff_tint_hexes"));
        &src[at..]
    } else {
        let at = src
            .find("_ => (\"#")
            .or_else(|| src.find("_ => ("))
            .unwrap_or_else(|| panic!("a dark arm in diff_tint_hexes"));
        &src[at..]
    };
    let mut hexes = arm
        .match_indices('"')
        .filter_map(|(i, _)| {
            let tail = &arm[i + 1..];
            let end = tail.find('"')?;
            let v = &tail[..end];
            v.starts_with('#').then(|| v.to_owned())
        })
        .collect::<Vec<_>>();
    hexes.resize(2, String::new());
    (hexes[0].clone(), hexes[1].clone())
}
