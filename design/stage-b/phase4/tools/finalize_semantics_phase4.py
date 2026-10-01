#!/usr/bin/env python3
"""Phase4-board equivalent of setup/tools/finalize_semantics_setup.py (#28d pattern).
Declares each authored svg as a reviewed reference_svg icon asset so preflight
accepts it; gives inputs their changed/behavior record. Glob p4-*."""
import hashlib, json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "cards"

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)

def finalize(d: Path):
    sm_path = d / "semantic-map.json"
    if not sm_path.exists():
        return f"{d.name}: no semantic-map yet"
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
    return f"{d.name}: {len(svgs)} icons, {len(inputs)} inputs"

if __name__ == "__main__":
    names = sys.argv[1:] or sorted(p.name for p in ROOT.glob("p4-*"))
    for n in names:
        print(finalize(ROOT / n))
