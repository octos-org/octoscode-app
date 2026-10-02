#!/usr/bin/env python3
"""Restore the AUTHORED palette colors into mapped.json (all 7 settings cards).

The map stage's ink sampler reads anti-aliased glyph pixels, so every text
color drifts per-glyph (the same spec muted #6E6E73 sampled as ff888b90 /
ff85888d / ff8f9399...), and light-on-dark labels sample their dark FILL
(07 "Stop server" ffd7161a, 12 "Take over" ff161819 — both authored white).
The authored palette IS the approved spec (ink #1D1D1F, muted #6E6E73,
accent #2F6FEB, white labels); restored verbatim. One exception: settings-06
t_stop's authored value was a bad pixel SAMPLE (fff9c2c2, a light pink —
samp() hit the row background); the map's measured ink ffdb2f32 is the real
reference red, so it is kept AND written back into the contract.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"

def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)

for k in range(6, 13):
    d = ROOT / f"settings-{k:02d}"
    contract = json.loads((d / "contract.json").read_text())
    authored = {n["id"]: n for n in walk(contract["tree"]) if n["t"] == "text"}
    mapped_path = d / "mapped.json"
    doc = json.loads(mapped_path.read_text())
    fixed = 0
    for n in walk(doc["tree"]):
        if n["t"] != "text":
            continue
        a = authored.get(n["id"])
        if not a:
            continue
        if k == 6 and n["id"] == "t_stop":
            continue  # keeps the measured reference red ffdb2f32 (authored was a bad sample)
        if a.get("color") is not None and n.get("color") != a["color"]:
            n["color"] = a["color"]
            fixed += 1
    mapped_path.write_text(json.dumps(doc, indent=2) + "\n")
    # write the measured red back into the contract so author==mapped==render
    if k == 6:
        for n in walk(contract["tree"]):
            if n["id"] == "t_stop":
                n["color"] = 0xFFDB2F32
        (d / "contract.json").write_text(json.dumps(contract, indent=2, ensure_ascii=False) + "\n")
    print(f"settings-{k:02d}: {fixed} colors restored")
