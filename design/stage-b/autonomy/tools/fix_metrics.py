#!/usr/bin/env python3
"""Correct the measured text metrics in `mapped.json` after the `map` stage.

Why (card #11c): `flows/image-lib/observe.py:182` derives the font size from the
OCR ink HEIGHT and then solves the residual width as `tracking`:

    size     = h / (y1 - y0)                 # h = OCR ink height (a LINE box)
    tracking = (w - (x1 - x0) * size) / (n-1) # residual = measured - tight bbox

`(y1 - y0)` is the font's TIGHT glyph bbox height (em) while the OCR `h` is the
line box (ascender..descender, plus antialiasing). Mixing the two over-estimates
`size` by ~1.30-1.45x, and all of the width error is then absorbed as tracking:
measured `tracking` lands at -0.06..-0.19 em (squashed, "Fixthestequeue") or up to
+0.11 em (spread, "O c t o s C o d e") instead of Inter's default ~0.

Measured on our v2 render (conversation-01, thread_1 row): the atlas has 28 glyph
runs at x-height 7 px; ours had 8 runs at 9 px - glyphs ~29% too tall and merged.

The design flow's own rule (`flows/image-lib/MAPPING-RULES.md:57`): "compare the
glyphs and word spaces, not only the bounding box. Choose a closer bundled face
before adding large positive or negative tracking."

Fix: fit the size to the measured ink WIDTH (the reliable, many-glyph measure) and
use Inter's default tracking (0). The height then follows the font's natural
proportions, so glyphs and word spaces match without artificial tracking.
drop `font_asc`/`font_desc` so `design.rs` uses its Inter-tuned default shift.

Run between the `map` and `semantic` stages. Idempotent.
"""
import json
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[1] / "cards"

# Rows whose OCR text begins with a glyph that is authored as a separate icon.
# Keyed by (scene number, node id): node ids repeat across scenes, so a global
# key would shift an unrelated scene's node (it did: conversation-11/t05).
# Value = the measured logical x where the copy itself starts.
X_OVERRIDE = {(4, "t05"): 87.5}   # conversation-04 tool_3: ">_ " is the terminal icon

# Nodes whose OCR row MERGED several visual runs, so the ink-width fit mis-sizes
# them and the authored size must stand (card #18b):
#   07 t02  — the green "+62" and the red "−5" arrive as ONE row ("+62 -5"), and
#             fitting 6 glyphs into the merged 31px ink box shrank them to 9.5pt
#             (the atlas draws both runs large). Authored as two explicit nodes.
#   07 t_undo — "Undo 9" merged the ↺ glyph into the label; fitting "Undo" into
#             the run's 61.5px ink box pushed it to 24pt (the atlas is ~14pt).
SIZE_KEEP = {(7, "t_undo")}

# Card #18d item 4: the OCR row MERGED the trailing chevron GLYPH into the label,
# so the ink-width fit ran the text under the icon ("Last turn v" fitted across
# 208.5..285.5 while the chevron sits at 277..285.5 — the glyph drew on the "n").
# Value = the logical x the label's ink must stop at.
X_RIGHT = {(11, "scope_label"): 268.0}

# Card #18e item 4: option 1 of the question card ("In the session ledger
# (recommended)") is ONE label whose OCR row is `missing_or_ocr_unresolved`, so it
# is never width-fitted — it kept the authored 14pt size and a stale 2-line-tall
# box (an empty gap under the single line) while its siblings fit to ~17.9pt. The
# atlas draws it on TWO lines (line 1 "In the session ledger", 181.49 wide; line 2
# "(recommended)"). Adopt a sibling's fitted size, wrap the label at the atlas's
# line-1 width, and take the height from the wrapped content (no gap, no bleed).
#   sibling = the already-fitted node whose size/line_height this node must match
#   wrap_w  = the label's measured box width (forces the atlas's 2-line break)
SIZE_FROM_SIBLING = {(6, "opt_ledger_label"): {"sibling": "opt_memory_label",
                                               "wrap_w": 181.49, "lines": 2}}

# Fonts must be resolved from the SAME tree compile.py validates against: the flow's
# repository('splash-makepad') = <native workspace>/octoscript-makepad. The mono face
# (ux/LiberationMono-Regular.ttf) is bundled in THIS clone, not in the read-only
# /native tree, so resolving there silently skips every mono row.
REPO = Path(__file__).resolve().parents[4]
FONT_ROOT = REPO / "tmp/stage-b/native-ws/octoscript-makepad/apps/kit-host"
_CACHE = {}


def metrics(path, text):
    font = _CACHE.get(path)
    if font is None:
        font = _CACHE[path] = TTFont(path)
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()
    units = font["head"].unitsPerEm
    cursor = 0
    bounds = []
    for ch in text:
        name = cmap.get(ord(ch), ".notdef")
        pen = BoundsPen(glyphs)
        glyphs[name].draw(pen)
        if pen.bounds:
            x0, y0, x1, y1 = pen.bounds
            bounds.append((x0 + cursor, y0, x1 + cursor, y1))
        cursor += font["hmtx"].metrics[name][0]
    box = [min(b[0] for b in bounds), min(b[1] for b in bounds),
           max(b[2] for b in bounds), max(b[3] for b in bounds)]
    return [v / units for v in box], cursor / units


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def font_path(src):
    return FONT_ROOT / src.removeprefix("self:")


def fix_scene(d, scene_no):
    obs_path = d / "observations.json"
    mapped_path = d / "mapped.json"
    if not obs_path.exists() or not mapped_path.exists():
        return 0
    obs = json.loads(obs_path.read_text())
    ink = {r["id"]: r.get("ink_bounds") for r in obs["text"]
           if r.get("status") == "observed" and r.get("ink_bounds")}
    doc = json.loads(mapped_path.read_text())
    # Card #18: the authored contract, so a FLOW region (dynamic runtime text) can
    # be restored to the box its author chose. The `map`/`observe` stage ink-fits
    # any node whose text matches one OCR row, which collapses a wrapping region to
    # a single line box (05 reason h 66 -> 23.5) — and `design.rs:338` then pins it
    # to the non-wrapping `flow: Right`. Frame text keeps that fitting.
    authored = {}
    if (d / "contract.json").exists():
        authored = {n["id"]: n for n in walk(json.loads((d / "contract.json").read_text())["tree"])
                    if n.get("t") == "text"}
    changed = 0
    for n in walk(doc["tree"]):
        if n["t"] != "text" or n["id"] not in ink:
            continue
        if n.get("variant") != "single_line":
            # a flow region: keep the author's box (width + multi-line height), and
            # drop the `map` stage's width-solved `tracking` — it was fitted to the
            # squashed size, so at the authored size it overlaps the glyphs.
            a = authored.get(n["id"])
            if a and a.get("h"):
                n["w"] = a["w"]
                n["h"] = a["h"]
                n["size"] = a.get("size", n.get("size"))
                n["line_height"] = a.get("line_height", n.get("line_height"))
                n["tracking"] = 0.0
                changed += 1
            continue
        fp = font_path(n["font_src"])
        if not fp.is_file():
            raise SystemExit(f"fix_metrics: font not found for {n['id']}: {fp}")
        (x0, _y0, _x1, _y1), advance = metrics(str(fp), n["text"])
        if advance <= 0:
            raise SystemExit(f"fix_metrics: non-positive advance for {n['id']}: {n['text']!r}")
        ix, iy, iw, ih = ink[n["id"]]
        # apple Vision reads the tool row's terminal ICON as a leading ">_ " glyph, so the
        # measured ink starts at the icon, not the copy. The icon is a separate SVG here;
        # shift the label to the measured text start and keep the measured right edge.
        if (scene_no, n["id"]) in X_OVERRIDE:
            new_x = X_OVERRIDE[(scene_no, n["id"])]
            iw = (ix + iw) - new_x
            ix = new_x
        if (scene_no, n["id"]) in X_RIGHT:
            # cap the ink's right edge (the merged chevron is a separate icon)
            iw = X_RIGHT[(scene_no, n["id"])] - ix
        if (scene_no, n["id"]) in SIZE_KEEP:
            # The OCR row merged a glyph into the text, so the ink-width fit is
            # wrong for this node: restore the AUTHORED size/box instead of leaving
            # the `map` stage's over-sized value (07 t_undo went to 27.75pt).
            a = authored.get(n["id"])
            if a:
                n["x"] = a["x"]
                n["y"] = a["y"]
                n["w"] = a["w"]
                n["h"] = a["h"]
                n["size"] = a["size"]
                n["line_height"] = a.get("line_height", n.get("line_height"))
                n["tracking"] = a.get("tracking", 0.0)
                changed += 1
            continue
        size = iw / advance                      # width-fit => tracking 0
        line_box = size * 2478 / 2048            # Inter's natural line box
        n["x"] = round(ix - x0 * size, 2)        # keep the glyph left-bearing offset
        n["y"] = round(iy - 2, 2)
        n["w"] = round(iw + 4, 2)
        # the box must be at least the line box, or preflight blocks it as clipping
        n["h"] = round(max(ih + 4, line_box), 2)
        n["size"] = round(size, 2)
        n["tracking"] = 0.0                      # Inter default
        # keep line_spacing at 1.0: line_height / line_box == 1
        n["line_height"] = round(line_box, 2)
        n["alignx"] = 0
        n.pop("font_asc", None)                  # let design.rs apply its Inter default
        n.pop("font_desc", None)
        changed += 1
    # Card #18e item 4: a node whose OCR row is `missing_or_ocr_unresolved` is
    # skipped above, so it keeps the authored size (14pt) and a stale 2-line-tall
    # box even though the label renders on one line (an empty gap under it). Adopt
    # a sibling's already-fitted size and take the height from one line box, so
    # option 1 wraps at the atlas's line-1 width and takes height from content.
    for (scene, nid), spec in SIZE_FROM_SIBLING.items():
        if scene != scene_no:
            continue
        sib = next((n for n in walk(doc["tree"]) if n["id"] == spec["sibling"]), None)
        tgt = next((n for n in walk(doc["tree"]) if n["id"] == nid), None)
        if not sib or not tgt:
            continue
        tgt["size"] = sib["size"]
        lh = sib.get("line_height", tgt.get("line_height"))
        tgt["line_height"] = lh
        tgt["tracking"] = 0.0
        # Wrap rather than clip: drop `single_line` and clamp the box to the
        # atlas's line-1 width so the label breaks at "(recommended)". Height is
        # the content's OWN line count (2), not a stale authored 2-line box.
        tgt["variant"] = None
        tgt["w"] = round(float(spec["wrap_w"]), 2)
        lines = spec["lines"]
        tgt["h"] = round(lh * lines, 2)
        changed += 1
    mapped_path.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


if __name__ == "__main__":
    total = 0
    for d in sorted(ROOT.glob("autonomy-*")):
        c = fix_scene(d, int(d.name.split("-")[1]))
        total += c
        print(f"{d.name}: corrected {c} text metrics")
    print(f"total {total}")
