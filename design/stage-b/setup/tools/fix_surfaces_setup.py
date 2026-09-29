#!/usr/bin/env python3
"""Restore surface fills/geometry in mapped.json against the reference (entry #28d).

Board-1's fix_surfaces.py restores authored fills only when a bg-child covers >1/3
(median contamination). The setup board's failures are different (measured from the
probe tables): `measure` snapped small surfaces onto the OCR text runs, so their
geometry AND whole-interior median are wrong even at 0-5% child cover — e.g. the
loading banner became a 275x44 grey box at (81,104) instead of the full-width amber
strip, and the page fill drifted to #F0F5F2. Rule here (evidence-based, cited):

  For each mapped node with `bg` that also exists in the contract:
    probe = 9-point mode of the reference inside the AUTHORED box (inset 18%)
    - if |probe - authored| <= TOL: restore authored bg (and geometry when the
      mapped box drifted >20 logical px on any edge)  [authored WAS a reference
      sample; the median/OCR-snap is the unreliable estimate]
    - elif |probe - mapped| > TOL: set bg = probe (a measured fill; e.g. the
      skills search box and install pills, where my authored sample hit a glyph)
Idempotent; run after `map` (before semantic), like board-1's fix scripts.
"""
import json
from collections import Counter
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1] / "cards"
TOL = 10          # per-channel tolerance for "same colour"
COVER = 0.33      # a bg-child this large dominates the whole-interior median
                  # (board-1 fix_surfaces.py rule; setup-09 bar_track went BLUE
                  # because bar_fill covers 65% of it)
MIN_AREA = 200.0  # logical px^2: below this, probe points are noise-dominated
                  # (setup-07 dot_glm is 11x11) — never probe-correct
DRIFT = 12        # logical px of edge drift that counts as geometry rewrite
INSET = 0.25      # probe inset inside the authored box (0.18 caught the
                  # palette scrim on the 08 modal edges)

def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)

def chans(v):
    return ((v >> 16) & 255, (v >> 8) & 255, v & 255)

def diff(a, b):
    return max(abs(x - y) for x, y in zip(a, b))

def probe_mode(ref, x, y, w, h):
    X, Y, W, H = int(x), int(y), int(w), int(h)
    ix, iy = int(W * INSET), int(H * INSET)
    pts = [(X+ix, Y+iy), (X+W-ix, Y+iy), (X+ix, Y+H-iy), (X+W-ix, Y+H-iy),
           (X+W//2, Y+iy), (X+W//2, Y+H-iy), (X+ix, Y+H//2), (X+W-ix, Y+H//2),
           (X+W//2, Y+H//2)]
    samples = []
    for px, py in pts:
        if 0 <= py*2 < ref.shape[0] and 0 <= px*2 < ref.shape[1]:
            samples.append(tuple(int(v) for v in ref[py*2, px*2]))
    if not samples:
        return None
    return Counter(samples).most_common(1)[0][0]

def fix_scene(d, k):
    cpath, mpath = d / "contract.json", d / "mapped.json"
    if not (cpath.exists() and mpath.exists()):
        return 0
    contract = {n["id"]: n for n in walk(json.loads(cpath.read_text())["tree"])}
    ref = __import__("numpy").array(
        __import__("PIL.Image", fromlist=["Image"]).open(d / "reference.png").convert("RGB")).astype(int)
    doc = json.loads(mpath.read_text())
    changed = 0
    for n in walk(doc["tree"]):
        a = contract.get(n["id"])
        if a is None or "bg" not in n:
            continue
        if "bg" not in a:
            continue
        # board-1 rule first: a big bg-child poisons the interior median/measure
        area = n["w"] * n["h"]
        cover = 0.0
        for g in walk(n):
            if g is not n and "bg" in g:
                cover = max(cover, (g["w"] * g["h"]) / area)
        # SIBLING-overlap guard: when another mapped surface (e.g. setup-09
        # bar_fill over bar_track - they are SIBLINGS, not parent/child) overlaps
        # this node's authored box, the 9-point probe lands on THAT surface's
        # paint and the "measured" branch would flip this node to the wrong
        # colour. Z-order is not in the data; trust the authored sample there.
        sib_cover = 0.0
        ox0, oy0, ox1, oy1 = a["x"], a["y"], a["x"]+a["w"], a["y"]+a["h"]
        for g in walk(doc["tree"]):
            if g is n or "bg" not in g:
                continue
            ix = max(0, min(ox1, g["x"]+g["w"]) - max(ox0, g["x"]))
            iy = max(0, min(oy1, g["y"]+g["h"]) - max(oy0, g["y"]))
            if ix > 0 and iy > 0:
                sib_cover = max(sib_cover, (ix*iy)/area if area else 0)
        if sib_cover > 0.4:
            # the 9-point probe lands on the OVERLAPPING sibling's paint, not on
            # this node's own (setup-09 bar_track under bar_fill): authored wins.
            if diff(chans(n["bg"]), chans(a["bg"])) > TOL:
                n["bg"] = a["bg"]
                changed += 1
                print(f"  setup-{k:02d}/{n['id']}: authored (sibling overlap {sib_cover:.2f})")
            continue
        if cover > COVER and diff(chans(n["bg"]), chans(a["bg"])) > TOL:
            n["bg"] = a["bg"]
            changed += 1
            print(f"  setup-{k:02d}/{n['id']}: authored (child cover {cover:.2f})")
            continue
        if area < MIN_AREA:
            continue
        mode = probe_mode(ref, a["x"], a["y"], a["w"], a["h"])
        if mode is None:
            continue
        acted = None
        trusted = diff(mode, chans(a["bg"])) <= TOL   # authored box verified against the reference
        if trusted and diff(chans(n["bg"]), chans(a["bg"])) > TOL:
            n["bg"] = a["bg"]
            acted = "authored"
        elif not trusted and diff(mode, chans(n["bg"])) > TOL and isinstance(n["bg"], int):
            n["bg"] = (0xFF000000 | (mode[0] << 16) | (mode[1] << 8) | mode[2])
            acted = "measured"
        # Geometry: the map/observe stage can snap a surface onto a nearby OCR run
        # (setup-12 skel1 became a 195x44 box at the THIRD row) even when its
        # whole-interior median bg still matches. If the authored box is verified,
        # restore drifted geometry regardless of whether the fill needed fixing.
        if trusted and (abs(n["x"]-a["x"]) > DRIFT or abs(n["y"]-a["y"]) > DRIFT
                        or abs(n["w"]-a["w"]) > DRIFT or abs(n["h"]-a["h"]) > DRIFT):
            n["x"], n["y"], n["w"], n["h"] = a["x"], a["y"], a["w"], a["h"]
            acted = (acted or "geometry") + "+geom"
        if acted:
            changed += 1
            print(f"  setup-{k:02d}/{n['id']}: {acted}")
    mpath.write_text(json.dumps(doc, indent=2) + "\n")
    return changed

if __name__ == "__main__":
    for k in range(7, 13):
        n = fix_scene(ROOT / f"setup-{k:02d}", k)
        print(f"setup-{k:02d}: {n} surfaces corrected")
