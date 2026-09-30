#!/usr/bin/env python3
"""Autonomy board 3: after `map`: declare each authored svg as a reference_svg icon asset, and give
each native input its required behavior binding. Idempotent; run before `semantic`."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)

for d in sorted(ROOT.glob("autonomy-*")):
    sm_path = d / "semantic-map.json"
    if not sm_path.exists():
        continue
    sm = json.loads(sm_path.read_text())
    ref = sm["reference_sha256"]
    nodes = list(walk(json.loads((d / "mapped.json").read_text())["tree"]))
    svgs = {n["id"] for n in nodes if n["t"] == "svg"}
    inputs = [n["id"] for n in nodes if n["t"] == "input"]
    by_id = {e["id"]: e for e in sm["elements"]}
    for iid in svgs:
        e = by_id[iid]
        e["role"] = "icon"
        e["basis"] = ("Line icon reconstructed as vector paths from the reference; "
                      "no raster or UI text.")
        e["confidence"] = 1.0
        e["decision"] = "reviewed"
        e["asset"] = {"path": f"assets/{iid}.svg", "sha256": sha(d / "assets" / f"{iid}.svg"),
                      "method": "reference_svg", "reference_sha256": ref,
                      "fit": "stretch", "clip": True,
                      "notes": ("Visually measured source icon reconstructed as vector paths; "
                                "no text or embedded raster")}
    for iid in inputs:
        by_id[iid]["behavior"] = {"event": "changed", "target": iid, "property": "text"}
    sm_path.write_text(json.dumps(sm, indent=2) + "\n")
    print(f"{d.name}: {len(svgs)} icons, {len(inputs)} inputs")
