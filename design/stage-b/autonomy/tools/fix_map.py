#!/usr/bin/env python3
"""Autonomy-specific corrections to `mapped.json` after the `map` stage.

`observe.py::map_observations` rewrites every OCR-matched text node from the
measured ink: x = ink start, color = median ink color. That is right for plain
copy, but wrong for nodes whose authored color is intentional (diff +/- code is
red/green, not the row's median ink) or whose authored x already accounts for a
leading icon (the OCR ink INCLUDES the bullet/icon glyph, so the refit x lands
on top of the separately-authored icon).

Fixes, keyed by node id (scene-scoped below):
- COLOR_KEEP: restore the authored contract color (diff code red/green, '}' ink).
- X_SHIFT: re-apply the authored +20 icon offset the ink-fit discarded.
Run between `map` and `fix_metrics` (fix_metrics re-fits size from ink width and
keeps x, so it must run AFTER this). Idempotent.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"

# node id -> authored color name in author_v2.C (restored verbatim from contract).
# This atlas draws the diff MARKER red/green but the CODE TEXT dark ink (v3
# side-by-side vs the atlas crop): map's median-ink rewrite lands near-black on
# every row either way, so restore each node's authored color.
COLOR_KEEP = {
    1: {"mk_2", "mk_3", "mk_4", "mk_5", "mk_6", "dl_2", "dl_3", "dl_4", "dl_5",
        "dl_6", "dl_7"},
}
# node id -> extra logical-x shift (icon occupies the first ~20px of the OCR ink)
X_SHIFT = {
    1: {"file_1_path": 20.0, "file_2_path": 20.0, "file_3_path": 20.0},
}


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def fix_scene(d, scene_no):
    mpath, cpath = d / "mapped.json", d / "contract.json"
    if not (mpath.exists() and cpath.exists()):
        return 0
    authored = {n["id"]: n for n in walk(json.loads(cpath.read_text())["tree"])}
    doc = json.loads(mpath.read_text())
    changed = 0
    for n in walk(doc["tree"]):
        nid = n.get("id")
        if nid in COLOR_KEEP.get(scene_no, set()) and nid in authored:
            n["color"] = authored[nid]["color"]
            changed += 1
        if nid in X_SHIFT.get(scene_no, {}):
            n["x"] = round(n["x"] + X_SHIFT[scene_no][nid], 2)
            n["w"] = round(max(n["w"] - X_SHIFT[scene_no][nid], 8), 2)
            changed += 1
    if changed:
        mpath.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


if __name__ == "__main__":
    total = 0
    for d in sorted(ROOT.glob("autonomy-*")):
        c = fix_scene(d, int(d.name.split("-")[1]))
        total += c
        print(f"{d.name}: corrected {c} map artifacts")
    print(f"total {total}")
