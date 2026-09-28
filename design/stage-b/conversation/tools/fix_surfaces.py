#!/usr/bin/env python3
"""Correct surface fills that `measure` contaminated with a large child.

Why (card #11e): `flows/image-lib/measure_surfaces.py:70-74` records a surface's
fill as ONE median over its whole interior (minus a 4px border). That is fine for
a panel that is a single flat colour, but wrong for a card that CONTAINS a large
differently-coloured child: conversation-04/tool_3 is a white, outlined card whose
console box (`tool_3_output`, grey) covers 61% of its area, so the median landed
on the child and the authored white was overwritten with panel grey (#f5f5f7).

The reference is unambiguous where the card's own paint is visible: tool_3's
interior outside the console box is white (255), not grey.

Fix: when a bg-bearing child covers more than a third of an authored surface's
area, restore the AUTHORED fill (the contract is the measured intent; the
aggregate median is the unreliable estimate). Idempotent; run after `map`.

The same aggregate-median rule is why tool_3's parent page also drifted; only
surfaces whose own fill is materially different from the child's are restored, so
a genuinely measured background (page white) is left untouched where it is right.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"
COVER = 0.33          # a child this large dominates a whole-interior median
DELTA = 8             # per-channel difference that counts as "materially different"


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def channels(v):
    return ((v >> 16) & 255, (v >> 8) & 255, v & 255)


def differs(a, b):
    return max(abs(x - y) for x, y in zip(channels(a), channels(b))) >= DELTA


def fix_scene(d):
    cpath, mpath = d / "contract.json", d / "mapped.json"
    if not (cpath.exists() and mpath.exists()):
        return 0
    contract = {n["id"]: n for n in walk(json.loads(cpath.read_text())["tree"])}
    doc = json.loads(mpath.read_text())
    changed = 0
    for n in walk(doc["tree"]):
        if "bg" not in n:
            continue
        authored = contract.get(n["id"], {}).get("bg")
        if authored is None:
            continue
        area = n["w"] * n["h"]
        if area <= 0:
            continue
        cover = 0.0
        for g in walk(n):
            if g is n or "bg" not in g:
                continue
            cover = max(cover, (g["w"] * g["h"]) / area)
        if cover > COVER and differs(n["bg"], authored):
            n["bg"] = authored
            changed += 1
    if changed:
        mpath.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


if __name__ == "__main__":
    total = 0
    for d in sorted(ROOT.glob("conversation-*")):
        n = fix_scene(d)
        total += n
        print(f"{d.name}: restored {n} surface fills")
    print(f"total {total} surface fills restored")
