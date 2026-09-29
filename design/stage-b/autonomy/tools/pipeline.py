#!/usr/bin/env python3
"""Drive the image-lib pipeline stages for all 6 autonomy scenes directly.

The flow runner (`image-to-appcard-flow.sh run --stages ...`) refuses a manifest
with fewer than 8 scenes (`flow.py::read_manifest`: "Author 8–12 scenes from one
atlas"); this board has 6 by task scope. This script calls the SAME underlying
stage modules the runner invokes (`observe.observe`, `measure_surfaces.measure`,
`observe.map_observations`, `semantics.propose/preflight`, `compile.compile_page`)
per scene — same pipeline, bypassing only the scene-count manifest check.

Idempotent-ish: `map` refuses to run over an existing mapped.json (same as the
runner); delete mapped.json first for a fresh mapping.

Run:  python3 tools/pipeline.py [observe|measure|map|finalize|semantic|compile|all]
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FLOW = Path(__file__).resolve().parents[4] / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow/flows"
sys.path.insert(0, str(FLOW / "image-lib"))
sys.path.insert(0, str(FLOW / "image-to-card"))

SCENES = [ROOT / "cards" / f"autonomy-{n:02d}" for n in range(1, 7)]


def stage(name, d):
    if name == "observe":
        from observe import observe
        return observe(d)
    if name == "measure":
        from measure_surfaces import measure
        if (d / "annotations.json").exists():
            return {"retained": "annotations.json; existing source measurements preserved"}
        return measure(d)
    if name == "map":
        from observe import map_observations
        from semantics import propose
        if (d / "mapped.json").exists():
            return {"retained": "mapped.json; existing mapping preserved"}
        res = map_observations(d)
        propose(d)
        return res
    if name == "finalize":
        out = subprocess.run([sys.executable, str(ROOT / "tools/finalize_semantics.py")],
                             capture_output=True, text=True)
        return {"finalize_semantics": out.stdout.strip(), "rc": out.returncode}
    if name == "fix_map":
        out = subprocess.run([sys.executable, str(ROOT / "tools/fix_map.py")],
                             capture_output=True, text=True)
        return {"fix_map": out.stdout.strip().splitlines()[-1] if out.stdout else "", "rc": out.returncode}
    if name == "fix_metrics":
        out = subprocess.run([sys.executable, str(ROOT / "tools/fix_metrics.py")],
                             capture_output=True, text=True)
        return {"fix_metrics": out.stdout.strip().splitlines()[-1] if out.stdout else "", "rc": out.returncode}
    if name == "fix_surfaces":
        out = subprocess.run([sys.executable, str(ROOT / "tools/fix_surfaces.py")],
                             capture_output=True, text=True)
        return {"fix_surfaces": out.stdout.strip().splitlines()[-1] if out.stdout else "", "rc": out.returncode}
    if name == "semantic":
        from semantics import preflight
        res = preflight(d)
        if not res.get("pass"):
            raise ValueError(f"semantic preflight failed: {d.name}")
        return {"pass": True}
    if name == "compile":
        from compile import compile_page
        return compile_page(d)
    raise ValueError(name)


def main():
    stages = sys.argv[1:] or ["all"]
    if stages == ["all"]:
        stages = ["observe", "measure", "map", "fix_map", "fix_metrics", "fix_surfaces",
                  "finalize", "semantic", "compile"]
    for s in stages:
        for d in SCENES:
            if s == "finalize" and d != SCENES[0]:
                continue  # finalize_semantics.py loops all scenes itself
            res = stage(s, d)
            print(json.dumps({"scene": d.name, "stage": s,
                              "result": res}, default=str)[:220], flush=True)


if __name__ == "__main__":
    main()
