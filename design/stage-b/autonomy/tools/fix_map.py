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
    # markers: keep the authored red/green. Code lines (dl_*): NOT listed, so the
    # map stage's measured dark ink stands (atlas draws code dark, markers coloured).
    1: {"mk_2", "mk_3", "mk_4", "mk_5", "mk_6",
        # dl_7 '}': single-glyph OCR ink samples the antialiased edge (#cacaca);
        # the atlas draws it dark (#4f4d51 measured). Restore the authored ink.
        "dl_7",
        # #28a3: the map stage's median-ink rewrite lands on the file rows'
        # ANTI-ALIASED grey (median 149) — the atlas path ink is near-black
        # (dark-core 73-84 measured). Restore the authored ink colour.
        "file_1_path", "file_2_path", "file_3_path", "diff_file_path"},
}
# node id -> restore the AUTHORED geometry (x, y, w, h) from the contract.
# Same disease as X_RESTORE but the map stage also re-anchors y/h on these
# single-row icons (it fits them to the row's OCR ink box), so restore the full
# authored box.
X_RESTORE_FULL = {
    1: {"review_panel", "mk_2", "mk_3", "mk_4", "mk_5", "mk_6",
        # #28a3 full-width band + rules: their authored geometry is measured off
        # the atlas (y 334.1/335.4/390.3, x0 w406) — do not let the map stage
        # re-anchor them to OCR/annotation bands.
        "diff_file_header", "diff_file_icon",
        "diff_band_rule_top", "diff_band_rule_bottom"},
    2: {"status_spinner",
        # #28a3 measured sizes (ink-height ratio): badge ~20.5, path ~19.5 —
        # pin the authored boxes so the ink-fit can't re-shrink them.
        "finding_high_badge_label", "finding_high_path",
        "finding_low_badge_label", "finding_low_path"},
    4: {"loop_1_pause", "loop_1_play", "loop_1_trash",
        "loop_2_pause", "loop_2_play", "loop_2_trash",
        "loop_3_play", "loop_3_trash",
        "loop_1_dot", "loop_2_dot", "loop_3_dot",
        # row text is CENTRED in the 122.25-logical row bands (name mid-10.4,
        # cadence mid+2.6) — the ink-fit re-pins y to the OCR box, undoing it.
        "loop_1_name", "loop_1_cad", "loop_2_name", "loop_2_cad",
        "loop_3_name", "loop_3_cad"},
    5: {"monitors_screen", "mon_1_pause", "mon_1_trash",
        "mon_2_pause", "mon_2_trash",
        # state and interval share one atlas ink band — both authored at 14.
        "mon_1_state", "mon_1_int", "mon_2_state", "mon_2_int"},
    6: {"peer_2_attn"},
}
# node id -> restore the AUTHORED geometry (x, w, size) from the contract.
# fix_metrics' ink-fit resets the path x to the OCR ink start (which INCLUDES
# the separately-authored file icon), so the path overlaps the icon, and it
# re-fits the font from that fused ink width. Restore the authored placement
# verbatim — self-healing, so re-running the stage never accumulates shifts
# (the earlier += shift form double-applied when mapped.json was retained).
X_RESTORE = {
    1: {"file_1_path", "file_2_path", "file_3_path", "diff_file_path",        # the map stage re-anchors each icon to its row's OCR ink start; row 3's
        # OCR box is indented, so its icon landed ON the path start (v-r2 crop).
        # Icons are authored at the measured x[28,46] ref band on every row —
        # restore that, not the per-row ink anchor.
        "file_1_icon", "file_2_icon", "file_3_icon", "diff_file_icon"},}
# node id -> max font size. The map stage's ink-fit over-sizes single-glyph
# markers; fix_metrics clamps mk_2..mk_5 but the 134-row marker (mk_6) slips
# through the map stage oversized. Clamp it here (fix_map runs after fix_metrics).
SIZE_CLAMP = {1: {"mk_6": 15.0, "mk_2": 15.0, "mk_3": 15.0, "mk_4": 15.0, "mk_5": 15.0}}


# node id -> restore the AUTHORED bg. `measure_surfaces` takes one interior
# median; on small or glyph-covered surfaces (thin progress bar, solid black
# pill under white text) the median lands on the page/label, erasing the fill.
BG_KEEP = {
    2: {"start_review_surface"},
    3: {"bar_track", "bar_fill", "pause_btn_surface"},
}
# node id -> restore the AUTHORED card style (bg/border/bordercolor). The round-2
# finding cards' border was invisible because measure rewrote the white/panel
# card bg to near-white and kept the hair border colour, which vanishes on it;
# the atlas card-edge luminance is ~220 = one step darker ("cardline").
STYLE_KEEP = {
    2: {"run_status_card", "finding_high", "finding_low"},
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
        if nid in SIZE_CLAMP.get(scene_no, {}):
            if n.get("size", 0) > SIZE_CLAMP[scene_no][nid]:
                n["size"] = SIZE_CLAMP[scene_no][nid]
                n["line_height"] = round(n["size"] * 2478 / 2048, 2)
                n["h"] = max(n["h"], n["line_height"])
                changed += 1
        if nid in BG_KEEP.get(scene_no, set()) and nid in authored and "bg" in n:
            n["bg"] = authored[nid]["bg"]
            changed += 1
        if nid in X_RESTORE.get(scene_no, set()) and nid in authored:
            for k in ("x", "w", "size"):
                if k in authored[nid]:
                    n[k] = authored[nid][k]
            if "size" in authored[nid]:
                n["line_height"] = round(authored[nid]["size"] * 2478 / 2048, 2)
                n["h"] = max(n.get("h", 0), n["line_height"])
            changed += 1
        if nid in X_RESTORE_FULL.get(scene_no, set()) and nid in authored:
            for k in ("x", "y", "w", "h", "size", "line_height"):
                if k in authored[nid]:
                    n[k] = authored[nid][k]
            changed += 1
        if nid in STYLE_KEEP.get(scene_no, set()) and nid in authored:
            for k in ("bg", "border", "bordercolor"):
                if k in authored[nid]:
                    n[k] = authored[nid][k]
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
