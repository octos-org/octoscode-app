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

# the flow's own metric routine, inlined so this script has no hidden dependency
FONT_ROOT = Path("/Users/yuechen/home/oa.noindex/native/octoscript-makepad/apps/kit-host")
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


def fix_scene(d):
    obs_path = d / "observations.json"
    mapped_path = d / "mapped.json"
    if not obs_path.exists() or not mapped_path.exists():
        return 0
    obs = json.loads(obs_path.read_text())
    ink = {r["id"]: r.get("ink_bounds") for r in obs["text"]
           if r.get("status") == "observed" and r.get("ink_bounds")}
    doc = json.loads(mapped_path.read_text())
    changed = 0
    for n in walk(doc["tree"]):
        if n["t"] != "text" or n["id"] not in ink:
            continue
        try:
            (x0, _y0, _x1, _y1), advance = metrics(str(font_path(n["font_src"])), n["text"])
        except Exception:
            continue
        if advance <= 0:
            continue
        ix, iy, iw, ih = ink[n["id"]]
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
    mapped_path.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


if __name__ == "__main__":
    total = 0
    for d in sorted(ROOT.glob("conversation-*")):
        c = fix_scene(d)
        total += c
        print(f"{d.name}: corrected {c} text metrics")
    print(f"total {total}")
