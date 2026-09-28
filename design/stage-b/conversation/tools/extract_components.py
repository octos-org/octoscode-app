#!/usr/bin/env python3
"""Card #18: compile the 7 reusable L0 components and place them at <repo>/design/components/.

`extract` (flows/image-to-card/extract.py) compiles each declared `cards[]` entry from
the scene's *native composition root* subtree into an independently mountable L0/kit
card. It requires `tree['t'] == 'stack'` (extract.py:49-50), which is why the scene
authoring wraps each component's subtree in a minimal `stack` (tools/author_v2.py
`Scene.wrap` / `wrap_card`).

The flow's `local()` refuses an output outside the project root (flow.py:33-40), so the
extract target must be the in-project `pipeline-output/service-cards` staging dir; this
script then places each `<owner>/<id>` folder at `design/components/<id>/` and writes the
`design/components/index.json` manifest. Idempotent.

Run:  python3 tools/extract_components.py   (after tools/rebuild.sh)
"""
import json
import os
import shutil
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]        # design/stage-b/conversation
ROOT = Path(__file__).resolve().parents[4]        # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
STAGE = HERE / "pipeline-output/service-cards"
OUT = ROOT / "design/components"
PY = "/Users/yuechen/miniconda3/bin/python3"

# Per-item bindings + the two Gate-B data variants, from card #18's component list.
SPEC = {
    "approval-card": {
        "scene": "5", "source": "conversation-05",
        "bindings": ["command", "reason", "action:approve", "action:approve_session", "action:deny"],
        "variants": {
            "short": {"command": "git push origin feat/steer-queue",
                      "reason": "Push the fix branch so CI can run"},
            "long": {"command": "cargo test -p octos-cli steer_queue -- --nocapture",
                     "reason": "Run the full steer-queue integration suite before pushing so a "
                               "regression in the durable queue is caught locally rather than in CI"}}},
    "question-card": {
        "scene": "6", "source": "conversation-06",
        "bindings": ["title", "question", "options", "note", "action:submit", "action:skip"],
        "variants": {
            "short": {"question": "Where should queued steers be persisted?"},
            "long": {"question": "Where should queued steers be persisted so they survive both "
                                 "a reconnect and an app restart without losing ordering?"}}},
    "edited-files-card": {
        "scene": "7", "source": "conversation-07",
        "bindings": ["count", "totals", "files", "action:undo", "action:review"],
        "variants": {
            "short": {"count": "Edited 1 file", "totals": "+12 -2"},
            "long": {"count": "Edited 3 files", "totals": "+62 -5"}}},
    "plan-card": {
        "scene": "10", "source": "conversation-10",
        "bindings": ["title", "steps"],
        "variants": {
            "short": {"title": "Plan \u00b7 1 of 2",
                      "steps": ["Reproduce reconnect drop", "Implement durable queue"]},
            "long": {"title": "Plan \u00b7 3 of 5",
                     "steps": ["Reproduce reconnect drop", "Trace steer queue lifecycle",
                               "Write failing test", "Implement durable queue",
                               "Run full suite and push"]}}},
    "goal-strip": {
        "scene": "10", "source": "conversation-10",
        "bindings": ["goal", "elapsed", "action:pause", "action:stop"],
        "variants": {
            "short": {"goal": "Goal \u00b7 Fix steer queue", "elapsed": "2m"},
            "long": {"goal": "Goal \u00b7 Fix steer queue on reconnect", "elapsed": "18m"}}},
    "diff-view": {
        "scene": "11", "source": "conversation-11",
        "bindings": ["scope", "file", "rows", "folded"],
        "variants": {
            # Card #18d item 2a: the OCR read the atlas's leading vertical-ellipsis
            # as a colon, so both variants carried a stray ':' and no marks; copy
            # scene 11's correct text.
            "short": {"file": "ui_protocol.rs  +9 -1", "folded": "⋮ 88 unmodified lines ⋮"},
            "long": {"file": "ui_protocol_transport.rs  +31 -4", "folded": "⋮ 412 unmodified lines ⋮"}}},
    "settings-group": {
        "scene": "12", "source": "conversation-12",
        "bindings": ["section", "rows", "model"],
        "variants": {
            "short": {"section": "Permissions", "model": "deepseek-v4-flash"},
            "long": {"section": "Permissions and defaults", "model": "deepseek-v4-flash"}}},
}


def run_extract():
    PROJ = HERE
    # `extract.py:25-26` refuses an existing output dir ("Retain reviewed standalone
    # cards"), so clear the staging dir to keep the run idempotent.
    if STAGE.exists():
        shutil.rmtree(STAGE)
    argv = ["bash", str(CLONE / "tools/image-to-appcard-flow.sh"), "run",
            "--project", str(PROJ), "--manifest", str(PROJ / "image-to-appcard-flow.json"),
            "--stages", "extract"]
    result = subprocess.run(argv, env={**os.environ, "BEAUTY_PYTHON": PY},
                            capture_output=True, text=True)
    tail = (result.stdout or result.stderr).strip().splitlines()[-2:]
    print("\n".join(tail))
    if result.returncode != 0:
        raise SystemExit("extract failed: " + (result.stderr or result.stdout)[-800:])


def main():
    run_extract()
    catalogue = json.loads((STAGE / "catalogue.json").read_text())
    if OUT.exists():
        # preserve the shared kit pack the L0 layer ships (card #17)
        keep = OUT / "native"
        tmp = OUT.parent / ".components-native-keep"
        if keep.exists():
            if tmp.exists():
                shutil.rmtree(tmp)
            shutil.copytree(keep, tmp)
        shutil.rmtree(OUT)
        OUT.mkdir(parents=True)
        if tmp.exists():
            shutil.copytree(tmp, OUT / "native")
            shutil.rmtree(tmp)
    else:
        OUT.mkdir(parents=True)
    index = {"schema_version": 1, "kind": "octoscode-l0-components",
             "note": "Reusable per-item components compiled from the approved conversation scenes "
                     "(card #18). Width-responsive: fill the slot width, height from content.",
             "components": []}
    for card in catalogue["cards"]:
        cid = card["id"]
        spec = SPEC[cid]
        src = STAGE / card["folder"]
        dest = OUT / cid
        shutil.copytree(src, dest)
        entry = {"id": cid, "source_scene": spec["source"],
                 "extracted_from": f"scene {card['scene']} root '{card['root']}'",
                 "owner": card["owner"], "bindings": spec["bindings"],
                 "variants": spec["variants"], "artboard": card["artboard"],
                 "nodes": card["nodes"], "native_controls": card["native_controls"],
                 "card": str((dest / "page.card").relative_to(ROOT)),
                 "data": str((dest / "page.data.json").relative_to(ROOT)),
                 "render": [str((dest / "reference.png").relative_to(ROOT))]}
        index["components"].append(entry)
    (OUT / "index.json").write_text(json.dumps(index, indent=2) + "\n")
    print(f"placed {len(index['components'])} components under {OUT.relative_to(ROOT)}/")


if __name__ == "__main__":
    main()
