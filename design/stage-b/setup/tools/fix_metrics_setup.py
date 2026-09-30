#!/usr/bin/env python3
"""Setup-board equivalent of conversation/tools/fix_metrics.py (entry #28d).

Same root cause (see board-1 docstring): observe.py:182-190 fits size to the OCR
ink HEIGHT and dumps the residual width into `tracking`, so mapped.json ships
squashed text (setup-07 t_title measured tracking -3.94 px + font_asc/desc).
Fix is verbatim: width-fit the size, tracking 0, drop font_asc/font_desc.
Only differences: cards glob (setup-*), and SIZE_KEEP for rows whose OCR text the
author REPLACED with different glyphs (the arrow hints of the command palette),
where the ink-width fit is invalid by construction.
"""
import json
from pathlib import Path
from fontTools.pens.boundsPen import BoundsPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[1] / "cards"
REPO = Path(__file__).resolve().parents[4]
FONT_ROOT = REPO / "tmp/stage-b/native-ws/octoscript-makepad/apps/kit-host"
_CACHE = {}
SIZE_KEEP = {(8, "t_hints"), (12, "t_banner"), (12, "t_retry")}   # authored text is "↑↓ to move · ↵ to run · esc";
                               # OCR read different glyphs, so ink width ≠ our advance

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
    authored = {}
    if (d / "contract.json").exists():
        authored = {n["id"]: n for n in walk(json.loads((d / "contract.json").read_text())["tree"])
                    if n.get("t") == "text"}
    changed = 0
    for n in walk(doc["tree"]):
        if n["t"] != "text" or n["id"] not in ink:
            continue
        if n.get("variant") != "single_line":
            n["tracking"] = 0.0
            changed += 1
            continue
        if (scene_no, n["id"]) in SIZE_KEEP:
            # board-1 semantics: the OCR row merged/replaced glyphs, so the
            # ink-width fit is invalid - restore the AUTHORED size and box.
            a = authored.get(n["id"])
            if a:
                n["x"], n["y"], n["w"], n["h"] = a["x"], a["y"], a["w"], a["h"]
                n["size"] = a["size"]
                n["line_height"] = a.get("line_height", n.get("line_height"))
            n["tracking"] = 0.0
            n.pop("font_asc", None)
            n.pop("font_desc", None)
            changed += 1
            continue
        fp = font_path(n["font_src"])
        if not fp.is_file():
            raise SystemExit(f"fix_metrics: font not found for {n['id']}: {fp}")
        (x0, _y0, _x1, _y1), advance = metrics(str(fp), n["text"])
        if advance <= 0:
            raise SystemExit(f"fix_metrics: non-positive advance for {n['id']}")
        ix, iy, iw, ih = ink[n["id"]]
        size = iw / advance
        line_box = size * 2478 / 2048
        n["x"] = round(ix - x0 * size, 2)
        n["y"] = round(iy - 2, 2)
        n["w"] = round(iw + 4, 2)
        n["h"] = round(max(ih + 4, line_box), 2)
        n["size"] = round(size, 2)
        n["tracking"] = 0.0
        n["line_height"] = round(line_box, 2)
        n["alignx"] = 0
        n.pop("font_asc", None)
        n.pop("font_desc", None)
        changed += 1
    mapped_path.write_text(json.dumps(doc, indent=2) + "\n")
    return changed

if __name__ == "__main__":
    for k in range(7, 13):
        d = ROOT / f"setup-{k:02d}"
        n = fix_scene(d, k)
        print(f"setup-{k:02d}: {n} text nodes re-fitted")
