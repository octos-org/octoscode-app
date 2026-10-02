//! Entry #36a — the attachments "68%" label must sit strictly INSIDE the upload
//! ring's inner hole, and be legible (the run is white over an arbitrary
//! thumbnail, and the card authors no scrim node).
//!
//! The hole is derived from the SERVED ring asset at runtime, never hardcoded:
//! `page.data.json` points `att2_ring` at `att2_ring-ba3ee197a8a5.svg`
//! (viewBox 24, `r=9.2`, `stroke-width=2.6`). The stroke is CENTRED on the path,
//! so the inner radius is (r - sw/2) scaled by ring_px/24.
//!
//! Measured on main before the fix (`/snap?all=1`, probe 8376):
//!   att2_ring [287, 250, 52, 52]   att2_pct [293, 263, 40, 25]
//! The 40.2 x 24.76 box has half-diagonal 23.61 against a hole radius of 17.12 —
//! every corner sat ~6.5px ON the stroke, and the authored seat (275.04, 294.73)
//! hung the entire label below the ring.

use std::sync::Arc;

use octoscode_store::Store;

/// The card dir, resolved from the manifest (a test's CWD is the crate dir, so
/// the repo-relative `design/…` path needs the manifest base — the same
/// `CARGO_MANIFEST_DIR` anchor the module's own `cards_root()` uses).
fn card_dir() -> String {
    format!(
        "{}/../../design/stage-b/autonomy/cards/autonomy-09",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn lowered_attachments() -> String {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    octoscode_module::screens::sessions::seed_attachments(vec![("a.png", 1024), ("b.png", 2048)]);
    octoscode_module::screens::sessions::lower_screen("attachments", &store).unwrap()
}

/// The inner-hole radius of the served ring SVG, in card px.
fn hole_radius() -> f64 {
    let dir = card_dir();
    let data: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{dir}/page.data.json")).unwrap(),
    )
    .unwrap();
    let src = data["$kit"]["placements"]["att2_ring"]["layout"]["src"]
        .as_str()
        .expect("att2_ring src in page.data.json")
        .rsplit('/')
        .next()
        .unwrap()
        .to_owned();
    let svg = std::fs::read_to_string(format!("{dir}/assets/{src}")).unwrap();
    let num = |needle: &str| -> f64 {
        let at = svg.find(needle).unwrap_or_else(|| panic!("{needle} in {src}"));
        let rest = &svg[at + needle.len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        rest[..end].parse().unwrap()
    };
    let r = num("r=\"");
    let sw = num("stroke-width=\"");
    let ring_px = 52.0; // the authored /snap ring rect is 52 x 52
    (r - sw / 2.0) * (ring_px / 24.0)
}

/// Read `width:`, `height:` and `abs_pos: vec2(x, y)` out of one lowered node.
fn node_rect(dsl: &str, node: &str) -> (f64, f64, f64, f64) {
    let npos = dsl
        .find(&format!("{node} := "))
        .unwrap_or_else(|| panic!("{node} present in the lowered DSL"));
    let rest = &dsl[npos..];
    let num_after = |key: &str| -> f64 {
        let at = rest.find(key).unwrap_or_else(|| panic!("{key} in {node}"));
        let tail = &rest[at + key.len()..];
        let end = tail
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(tail.len());
        tail[..end].parse().unwrap()
    };
    let vp = rest.find("abs_pos: vec2(").expect("abs_pos in node");
    let vtail = &rest[vp + "abs_pos: vec2(".len()..];
    let close = vtail.find(')').unwrap();
    let mut it = vtail[..close].split(',');
    let x: f64 = it.next().unwrap().trim().parse().unwrap();
    let y: f64 = it.next().unwrap().trim().parse().unwrap();
    (num_after("width: "), num_after("height: "), x, y)
}

fn ring_centre() -> (f64, f64) {
    let (w, h, x, y) = node_rect(&lowered_attachments(), "att2_ring");
    assert_eq!((w, h), (52.0, 52.0), "the ring box must stay the authored 52x52");
    (x + w / 2.0, y + h / 2.0)
}

/// THE PASS TEST (the entry's): the label rect lies strictly inside the ring's
/// inner hole — every corner inside the circle, with clearance. On main the
/// half-diagonal is 23.61 vs a 17.12 hole, so this fails there.
#[test]
fn the_pct_label_lies_strictly_inside_the_rings_hole() {
    let r = hole_radius();
    let (w, h, x, y) = node_rect(&lowered_attachments(), "att2_pct");
    let (cx, cy) = ring_centre();
    let clearance = 1.0;
    for (name, (px, py)) in [
        ("TL", (x, y)),
        ("TR", (x + w, y)),
        ("BL", (x, y + h)),
        ("BR", (x + w, y + h)),
    ] {
        let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
        assert!(
            d <= r - clearance,
            "label corner {name} ({px:.2},{py:.2}) is {d:.2}px from the ring centre \
             (313, 276) — outside the hole radius {r:.2} with {clearance}px clearance"
        );
    }
    eprintln!("hole R={r:.3}  label {w:.2}x{h:.2} at ({x:.2},{y:.2})  centre ({cx:.1},{cy:.1})");
}

/// The label is centred on the ring, not merely near it.
#[test]
fn the_pct_label_centres_on_the_ring() {
    let (w, h, x, y) = node_rect(&lowered_attachments(), "att2_pct");
    let (cx, cy) = ring_centre();
    assert!(
        (x + w / 2.0 - cx).abs() < 0.2 && (y + h / 2.0 - cy).abs() < 0.2,
        "label centre ({:.2},{:.2}) must sit on the ring centre ({cx:.2},{cy:.2})",
        x + w / 2.0,
        y + h / 2.0
    );
}

/// Legibility: a dark backing disc is painted in the hole BEHIND the label, so
/// the white run always has contrast over any thumbnail. On main `att_2` has no
/// scrim child at all, so this fails there.
#[test]
fn the_pct_label_has_a_dark_backing_inside_the_hole() {
    let dsl = lowered_attachments();
    // node NAME, not its kind: the scrim is emitted as a DesignSurface (a bare
    // View lays the rect out but paints no background in this vocabulary).
    let scrim = "att2_pct_scrim := ";
    let at = dsl
        .find(scrim)
        .expect("a dark scrim must be painted behind the label");
    // behind the label, not after it
    let label_at = dsl.find("att2_pct := ").unwrap();
    assert!(
        at < label_at,
        "the scrim must be declared BEFORE att2_pct so the white run draws on top"
    );
    // 400 covers the full multi-line DesignSurface head (the bare View head was
    // short, but the surface carries the radius/ellipse/border/flow props too,
    // so `draw_bg.color` sits past 260).
    let head = &dsl[at..(at + 400).min(dsl.len())];
    let colour = head
        .find("draw_bg.color: #")
        .map(|i| &head[i + 16..i + 24])
        .unwrap_or("");
    // the DSL writes #rrggbbaa (8 hex digits)
    assert_eq!(colour.len(), 8, "scrim needs an #rrggbbaa fill, got {colour:?}");
    let hex = |start: usize, end: usize| {
        let s = &colour[start..end];
        match u8::from_str_radix(s, 16) {
            Ok(v) => v,
            Err(e) => panic!("bad hex {colour:?} at {start}..{end} ({s:?}): {e}"),
        }
    };
    let rgb = [hex(0, 2), hex(2, 4), hex(4, 6)];
    // dark enough behind white text
    let lum = 0.2126 * rgb[0] as f64 + 0.7152 * rgb[1] as f64 + 0.0722 * rgb[2] as f64;
    assert!(lum < 90.0, "scrim must be dark (luma {lum:.1}), got {colour:?}");
    assert!(
        head.contains("draw_bg.ellipse: 1.0"),
        "the backing must be a disc filling the circular hole"
    );
    // and it must fill the hole, not overshoot the stroke. A disc's CORNER is
    // necessarily at r*sqrt(2) from the centre (17.115*1.414 = 24.21), so the
    // quantity to bound is the disc's half-size (its radius), not the corner.
    let (w, h, x, y) = node_rect(&dsl, "att2_pct_scrim");
    let (cx, cy) = ring_centre();
    assert!(
        (x + w / 2.0 - cx).abs() < 0.2 && (y + h / 2.0 - cy).abs() < 0.2,
        "the scrim must centre on the ring"
    );
    let r = hole_radius();
    assert!(
        w / 2.0 <= r + 0.2 && h / 2.0 <= r + 0.2,
        "the scrim disc (⌀{w:.2}) must not overshoot the hole radius {r:.2}"
    );
}

/// The run must shrink with its box, or it still rides the stroke even though
/// the box fits (the lowered font_size was 11.8875 with a 40.2-wide box).
#[test]
fn the_pct_run_is_sized_to_its_box() {
    let dsl = lowered_attachments();
    let npos = dsl.find("att2_pct := ").expect("att2_pct");
    // A1: the head window grew from 700 — the kit's CJK member is now the
    // bundled sans face plus a lazy fallback (design::cjk_members), which
    // pushed `font_size` past 700 chars of the font family.
    let head = &dsl[npos..(npos + 1600).min(dsl.len())];
    let at = head
        .find("font_size: ")
        .unwrap_or_else(|| panic!("font_size in the lowered label head: {head}"));
    let tail = &head[at + "font_size: ".len()..];
    let end = tail
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap();
    let size: f64 = tail[..end].parse().unwrap();
    // The run/font ratio must come from a MEASURED render, not the authored card
    // (40.2/15.85 = 2.5363) and not a constant scaled across sizes. Two rounds of
    // capture show the ratio is NOT constant — small-size hinting/AA:
    //     11.45pt -> 28.50px ink (2.4891) but that run was itself CLIPPED
    //              (0.02px left margin), so 2.4891 is only a LOWER bound;
    //     10.00pt -> 28.00px ink (2.8000) with zero ink outside its box.
    // Only the second measurement is unclipped, so 2.80 is the value to guard
    // with: it is the widest run observed per point of font size, and it is the
    // one measured where nothing was cut. Guards: the run must fit the box, and
    // the box must keep real headroom (a zero-margin fit is a clipped glyph).
    const MEASURED_RATIO: f64 = 28.00 / 10.00;
    let run = size * MEASURED_RATIO;
    let (w, _, _, _) = node_rect(&dsl, "att2_pct");
    assert!(
        run <= w,
        "the {size:.2}pt run ({run:.2}px at the unclipped measured ratio {MEASURED_RATIO:.4}) \
         must fit its {w:.2}px box — a flush run is a clipped glyph"
    );
    // and there must be real clip HEADROOM, not a zero-margin fit
    assert!(
        w - run >= 1.0,
        "the box needs >= 1.0px of headroom past the run (box {w:.2}, run {run:.2})"
    );
    // the box must also stay inside the hole — the two constraints bind together
    let (bw, bh, _, _) = node_rect(&dsl, "att2_pct");
    let half_diag = ((bw / 2.0).powi(2) + (bh / 2.0).powi(2)).sqrt();
    assert!(
        half_diag <= hole_radius() - 1.0,
        "box half-diagonal {half_diag:.2} must stay >= 1.0px inside the hole radius {:.2}",
        hole_radius()
    );
    assert!(
        size < 11.8875,
        "the run must shrink from main's 11.8875pt (was {size:.2})"
    );
}

/// Machine-readable geometry beside the human doc (RULES: CSV with a header).
#[test]
fn write_the_geometry_row() {
    let r = hole_radius();
    let dsl = lowered_attachments();
    let (w, h, x, y) = node_rect(&dsl, "att2_pct");
    let (cx, cy) = ring_centre();
    let mut far: f64 = 0.0;
    for (px, py) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
        far = far.max(((px - cx).powi(2) + (py - cy).powi(2)).sqrt());
    }
    let row = format!(
        "card,node,label_w,label_h,label_x,label_y,ring_cx,ring_cy,hole_r,farthest_corner,inside\n\
         autonomy-09,att2_pct,{w:.2},{h:.2},{x:.2},{y:.2},{cx:.1},{cy:.1},{r:.3},{far:.3},{}\n",
        far <= r - 1.0
    );
    let path = format!("{}/docs/attachments-ring-geometry.csv", env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/octoscode-module"));
    if let Some(dir) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&path, &row).unwrap();
    println!("{row}");
}
