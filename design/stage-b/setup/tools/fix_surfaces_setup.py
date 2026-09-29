#!/usr/bin/env python3
"""Restore surface fills/geometry in mapped.json against the reference (entry #28d).

Evidence-based rules, each earned on this board (see report-28d.md):
1. CHILD-COVER: a bg-child covering >1/3 poisons measure's whole-interior median
   (board-1 fix_surfaces.py rule). Restore the authored fill.
2. TINY surfaces (< 200 logical px^2, e.g. setup-07 dot_glm at 11x11): the 9-point
   probe is noise-dominated and the median is contaminated by the surround.
   Authored wins for bg AND geometry, unconditionally.
3. PAGE ROOT: the fill is a palette token, not a pixel estimate. Probe points land
   on whatever big chrome sits on the page (setup-11: 3 of 9 points fell inside
   the black Reload pill and flipped the mode to black). Authored wins.
4. SIBLING-OVERLAP > 0.4: another mapped surface covers the authored box
   (setup-09 bar_fill over bar_track - siblings, not parent/child), so the probe
   lands on THAT paint. Authored wins.
5. Otherwise: trusted = probe ~ authored fill. If trusted and mapped drifted,
   restore authored; if NOT trusted and probe matches neither, adopt the probe
   (a genuinely measured fill, e.g. the setup-10 search pill).
6. GEOMETRY: the map/observe stage snaps small surfaces onto OCR rows
   (setup-10 search_pill was relocated onto the Install row at (269,476)).
   Restore drifted geometry whenever the authored box is probe-verified (trusted),
   BEFORE any sibling-continue can skip it.
Idempotent; run after `map` (with fix_metrics), before `semantic`.
"""
import json
from collections import Counter
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1] / "cards"
TOL = 10          # per-channel tolerance for "same colour"
DRIFT = 12        # logical px of edge drift that counts as geometry rewrite
INSET = 0.25      # probe inset inside the authored box (0.18 caught the palette
                  # scrim on the 08 modal edges)
COVER = 0.33      # child-cover fraction that poisons a whole-interior median
SIB = 0.40        # sibling-overlap fraction that blinds the probe
MIN_AREA = 200.0  # logical px^2 below which probing is noise-dominated


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
    pts = [(X + ix, Y + iy), (X + W - ix, Y + iy), (X + ix, Y + H - iy),
           (X + W - ix, Y + H - iy), (X + W // 2, Y + iy), (X + W // 2, Y + H - iy),
           (X + ix, Y + H // 2), (X + W - ix, Y + H // 2), (X + W // 2, Y + H // 2)]
    samples = []
    for px, py in pts:
        if 0 <= py * 2 < ref.shape[0] and 0 <= px * 2 < ref.shape[1]:
            samples.append(tuple(int(v) for v in ref[py * 2, px * 2]))
    if not samples:
        return None
    return Counter(samples).most_common(1)[0][0]


def fix_scene(d, k):
    cpath, mpath = d / "contract.json", d / "mapped.json"
    if not (cpath.exists() and mpath.exists()):
        return 0
    contract = {n["id"]: n for n in walk(json.loads(cpath.read_text())["tree"])}
    ref = np.array(Image.open(d / "reference.png").convert("RGB")).astype(int)
    doc = json.loads(mpath.read_text())
    nodes = list(walk(doc["tree"]))
    changed = 0
    for n in nodes:
        a = contract.get(n["id"])
        if a is None or "bg" not in n or "bg" not in a:
            continue
        area = n["w"] * n["h"]
        acted = None

        # rule 1: child-cover poisons the median
        cover = 0.0
        for g in walk(n):
            if g is not n and "bg" in g:
                cover = max(cover, (g["w"] * g["h"]) / area if area else 0)
        if cover > COVER and diff(chans(n["bg"]), chans(a["bg"])) > TOL:
            n["bg"] = a["bg"]
            acted = f"authored (child cover {cover:.2f})"

        # rule 2: tiny surfaces - authored wins outright
        if acted is None and area < MIN_AREA:
            if diff(chans(n["bg"]), chans(a["bg"])) > TOL:
                n["bg"] = a["bg"]
                acted = "authored (tiny)"
            n["x"], n["y"], n["w"], n["h"] = a["x"], a["y"], a["w"], a["h"]

        # probe the authored box
        mode = probe_mode(ref, a["x"], a["y"], a["w"], a["h"])
        trusted = mode is not None and diff(mode, chans(a["bg"])) <= TOL

        # rule 3: the page root's fill is a palette token
        if n["id"] == "page":
            trusted = True

        # sibling overlap blinds the probe (rule 4)
        sib_cover = 0.0
        ox0, oy0, ox1, oy1 = a["x"], a["y"], a["x"] + a["w"], a["y"] + a["h"]
        for g in nodes:
            if g is n or "bg" not in g:
                continue
            ix = max(0, min(ox1, g["x"] + g["w"]) - max(ox0, g["x"]))
            iy = max(0, min(oy1, g["y"] + g["h"]) - max(oy0, g["y"]))
            if ix > 0 and iy > 0:
                sib_cover = max(sib_cover, (ix * iy) / area if area else 0)

        # rule 6: geometry restore before any continue. A blinding sibling also
        # voids the probe for GEOMETRY (setup-10 search_pill: the pill's own light
        # label ink flipped the mode, so "trusted" went False while the box sat
        # relocated on the Install row) - authored is then the only evidence.
        geo_trusted = trusted or sib_cover > SIB
        if geo_trusted and (abs(n["x"] - a["x"]) > DRIFT or abs(n["y"] - a["y"]) > DRIFT
                            or abs(n["w"] - a["w"]) > DRIFT or abs(n["h"] - a["h"]) > DRIFT):
            n["x"], n["y"], n["w"], n["h"] = a["x"], a["y"], a["w"], a["h"]
            acted = (acted or "geometry") + "+geom"

        # fill decisions
        if acted is None:
            if sib_cover > SIB:
                if diff(chans(n["bg"]), chans(a["bg"])) > TOL:
                    n["bg"] = a["bg"]
                    acted = f"authored (sibling overlap {sib_cover:.2f})"
            elif trusted and diff(chans(n["bg"]), chans(a["bg"])) > TOL:
                n["bg"] = a["bg"]
                acted = "authored"
            elif not trusted and mode is not None and diff(mode, chans(n["bg"])) > TOL:
                n["bg"] = 0xFF000000 | (mode[0] << 16) | (mode[1] << 8) | mode[2]
                acted = "measured"

        if acted:
            changed += 1
            print(f"  setup-{k:02d}/{n['id']}: {acted}")
    mpath.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


if __name__ == "__main__":
    total = 0
    for k in range(7, 13):
        n = fix_scene(ROOT / f"setup-{k:02d}", k)
        total += n
        print(f"setup-{k:02d}: {n} surfaces corrected")
    print(f"total {total}")
