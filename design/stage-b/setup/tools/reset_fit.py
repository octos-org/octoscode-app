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


changed = 0
for d in sorted(ROOT.glob("setup-*")):
    contract = d / "contract.json"
    mapped = d / "mapped.json"
    if not (contract.exists() and mapped.exists()):
        continue
    authored = {n["id"]: n for n in walk(json.loads(contract.read_text())["tree"])
                if n["t"] == "text"}
    tree = json.loads(mapped.read_text())
    hit = 0
    for n in walk(tree["tree"]):
        if n["t"] != "text":
            continue
        a = authored.get(n["id"])
        if not a:
            continue
        # restore the FULL authored box + typography: map also rewrites h/x/y
        # (leaving h behind made boxes shorter than their line box and
        # preflight fails with "text box height ... is under its line box").
        for k in ("x", "y", "w", "h", "size", "line_height"):
            if k in a and n.get(k) != a[k]:
                n[k] = a[k]
                hit += 1
        for k in FIT_KEYS:
            n.pop(k, None)
    if hit:
        mapped.write_text(json.dumps(tree, indent=2) + "\n")
        changed += 1
    print(f"{d.name}: {hit} metric fixes")
print(f"reset_fit: {changed} cards rewritten")
