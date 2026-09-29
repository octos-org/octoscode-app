#!/usr/bin/env python3
"""#28c: undo the map stage's bad font fit on the setup cards.

`map` (fit-reference-fonts) rewrote the six setup trees' text metrics to
size 21-24 / tracking ±2.4-4.4 with drifting line boxes (e.g. setup-06
t_g1_r1 h 25 vs siblings 20.5), which rendered as doubled/ghosted text.
The board-1 cards that passed Gate B kept `tracking: 0.0` there (A/B:
conversation-12 mapped.json), and the authored metrics already match the
measured OCR boxes by construction — so restore every text node's
size/line_height from the authored contract and drop the fit fields.
Idempotent; run after `map`, before `semantic`/`compile`.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"
FIT_KEYS = ("tracking", "font_asc", "font_desc")


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


WIDEN_SKIP = {"t_last", "retry_label", "t_sb_b", "t_sb_r"}

changed = 0
for d in sorted(ROOT.glob("setup-*")):
    contract = d / "contract.json"
    mapped = d / "mapped.json"
    if not (contract.exists() and mapped.exists()):
        continue
    authored = {n["id"]: n for n in walk(json.loads(contract.read_text())["tree"])}
    tree = json.loads(mapped.read_text())
    hit = 0
    for n in walk(tree["tree"]):
        a = authored.get(n["id"])
        if not a:
            continue
        if n["t"] == "text":
            # fit's SIZE was right (the reference glyph box ≈ the font size); the
            # poison was tracking ±3-4 and drifting line boxes (the v2 ghosting).
            # So: keep the fit size (clamped), keep the authored geometry, and
            # restate line_height/h from the size invariant so preflight's
            # box>=line-box check holds while the glyphs grow to match the target.
            # outer-loop round 2: the fit sizes track the reference but render
            # 15-20% small, so scale the kept fit size up and restate the box.
            # round-2 v9: the authored sizes ARE the class-correct targets
            # (apply_type_scale); the fit sizes are loose Vision boxes and any
            # blanket ratio overshoots (v8 labels ~21px vs atlas ~15).
            size = a.get("size") or n.get("size")
            if size:
                n["size"] = size
            lh = max(float(size) * 1.5, float(a.get("line_height") or 0))
            n["line_height"] = lh
            n["h"] = max(float(a.get("h") or 0), lh)
            # glyphs got 18% wider: widen the single-line box or the label clips
            # at its own box edge (v7: "Serv", "Conne", "Token rejected by t").
            if n["id"] not in WIDEN_SKIP:
                n["w"] = round(float(a.get("w") or 0) * 1.3, 2)
            for k in ("x", "y"):
                if k in a:
                    n[k] = a[k]
            for k in FIT_KEYS:
                n.pop(k, None)
            hit += 1
        else:
            # map's median sampling repaints surface fills (the grey #F0F5F2
            # page bug): restore every authored paint/geometry field.
            for k in ("x", "y", "w", "h", "color", "bg", "radius", "border", "bordercolor"):
                if k in a and n.get(k) != a[k]:
                    n[k] = a[k]
                    hit += 1
    if hit:
        mapped.write_text(json.dumps(tree, indent=2) + "\n")
        changed += 1
    print(f"{d.name}: {hit} metric fixes")
print(f"reset_fit: {changed} cards rewritten")
