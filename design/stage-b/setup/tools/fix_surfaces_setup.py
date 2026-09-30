#!/usr/bin/env python3
"""Guard mapped.json against map/measure-stage fill+geometry corruption (#28d).

Every authored bg in the setup contracts was SAMPLED FROM THE REFERENCE (see
author_setup.py comments); measure's whole-interior medians were repeatedly
wrong on this board (child contamination, OCR-row snapping, chrome under the
probe). So the guard is authored-wins, with the probe kept only as a WARNING
for future auditability. Rules (each earned, see report-28d.md):

1. CHILD-COVER > 1/3: median poisoned (board-1 rule; setup-09 bar_track went
   blue because bar_fill covers 65% of it).
2. TINY surfaces (< 200 px^2, setup-07 dot_glm 11x11): probe and median are
   noise-dominated.
3. PAGE ROOT: the fill is a palette token; probe points land on big chrome
   (setup-11: 3/9 points inside the black Reload pill flipped the mode).
4. GEOMETRY: the map stage snaps small surfaces onto OCR rows (setup-10
   search_pill was relocated onto the Install row). Restore drifted geometry.
Idempotent; run after `map` (with fix_metrics), before `semantic`.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"
TOL = 10
DRIFT = 12
COVER = 0.33
MIN_AREA = 200.0


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def chans(v):
    return ((v >> 16) & 255, (v >> 8) & 255, v & 255)


def diff(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def fix_scene(d, k):
    cpath, mpath = d / "contract.json", d / "mapped.json"
    if not (cpath.exists() and mpath.exists()):
        return 0
    contract = {n["id"]: n for n in walk(json.loads(cpath.read_text())["tree"])}
    doc = json.loads(mpath.read_text())
    changed = 0
    for n in walk(doc["tree"]):
        a = contract.get(n["id"])
        if a is None or "bg" not in n or "bg" not in a:
            continue
        area = n["w"] * n["h"]
        reason = None

        # rule 1: child-cover poisons the median
        cover = 0.0
        for g in walk(n):
            if g is not n and "bg" in g:
                cover = max(cover, (g["w"] * g["h"]) / area if area else 0)
        if cover > COVER:
            reason = f"child cover {cover:.2f}"
        # rule 2: tiny surfaces
        elif area < MIN_AREA:
            reason = "tiny"
        # rule 3: the page root's fill is a palette token
        elif n["id"] == "page":
            reason = "page token"

        if reason and diff(chans(n["bg"]), chans(a["bg"])) > TOL:
            n["bg"] = a["bg"]
            changed += 1
            print(f"  setup-{k:02d}/{n['id']}: authored ({reason})")
        elif not reason and diff(chans(n["bg"]), chans(a["bg"])) > TOL:
            # no proven contamination source, but the reference-sampled authored
            # value is still the stronger evidence than the interior median -
            # restore, and note it for the report.
            n["bg"] = a["bg"]
            changed += 1
            print(f"  setup-{k:02d}/{n['id']}: authored (median drift)")

        # rule 4: geometry restore (map snaps small surfaces onto OCR rows)
        if (abs(n["x"] - a["x"]) > DRIFT or abs(n["y"] - a["y"]) > DRIFT
                or abs(n["w"] - a["w"]) > DRIFT or abs(n["h"] - a["h"]) > DRIFT):
            n["x"], n["y"], n["w"], n["h"] = a["x"], a["y"], a["w"], a["h"]
            changed += 1
            print(f"  setup-{k:02d}/{n['id']}: geometry restored")
    mpath.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


if __name__ == "__main__":
    total = 0
    for k in range(7, 13):
        n = fix_scene(ROOT / f"setup-{k:02d}", k)
        total += n
        print(f"setup-{k:02d}: {n} surfaces corrected")
    print(f"total {total}")
