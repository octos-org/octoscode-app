#!/usr/bin/env python3
"""Card #16: compile the 9 reusable L0 components and place them at <repo>/design/components/.

`extract` (flows/image-to-card/extract.py) compiles each declared `cards[]` entry from
the scene's *native composition root* subtree into an independently mountable L0/kit
card. It requires `tree['t'] == 'stack'` (extract.py:49-50), which is why the three
non-stack roots (icon_spinner svg, answer_md markdown text, icon_copy svg) are wrapped
in a minimal `stack` in tools/author_v2.py first.

The flow's `local()` refuses an output outside the project root (flow.py:33-40), so the
extract target must be the in-project `pipeline-output/service-cards` staging dir; this
script then places each `<owner>/<id>` folder at `design/components/<id>/` and writes the
`design/components/index.json` manifest that card #16 step 4 asks for. Idempotent.

Run:  python3 tools/extract_components.py   (after tools/rebuild.sh)
"""
import json
import os
import shutil
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]        # design/stage-b/conversation
ROOT = Path(__file__).resolve().parents[4]        # repo root (p0-harness)
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
STAGE = HERE / "pipeline-output/service-cards"
OUT = ROOT / "design/components"
PY = "/Users/yuechen/miniconda3/bin/python3"

# Per-item bindings + the two Gate-B data variants, from card #16's table.
SPEC = {
    "thread-row": {"scene": "1", "source": "conversation-01", "bindings": ["title", "selected", "forked",
                   "action:open"], "variants": {"short": {"title": "Add session fork", "selected": False},
                   "long": {"title": "Bump octos-core to a6ea8505 and re-verify the steer queue timeout",
                            "selected": True}}},
    "new-chat": {"scene": "1", "source": "conversation-01", "bindings": ["action:new_chat"],
                 "variants": {"short": {"label": "New chat"}, "long": {"label": "New chat"}}},
    "user-bubble": {"scene": "3", "source": "conversation-03", "bindings": ["text"],
                    "variants": {"short": {"text": "Retry the build"},
                                 "long": {"text": "Fix the steer queue so queued steers survive a reconnect"}}},
    "working-row": {"scene": "3", "source": "conversation-03", "bindings": ["label", "elapsed"],
                    "variants": {"short": {"label": "Working", "elapsed": "3s"},
                                 "long": {"label": "Working", "elapsed": "12s"}}},
    "assistant-prose": {"scene": "9", "source": "conversation-09", "bindings": ["markdown"],
                        "variants": {"short": {"markdown": "Fixed `steer_dropped` handling."},
                                     "long": {"markdown": "Queued steers now survive a reconnect.\n\n"
                                              "\u2022 Updated `ui_protocol_transport.rs` to persist queued steers.\n\n"
                                              "\u2022 All tests pass: `12 passed`."}}},
    "worked-for": {"scene": "9", "source": "conversation-09", "bindings": ["label", "action:toggle"],
                   "variants": {"short": {"label": "Worked for 3s \u203a"},
                                "long": {"label": "Worked for 3m 4s \u203a"}}},
    "answer-actions": {"scene": "9", "source": "conversation-09", "bindings": ["timestamp", "actions"],
                       "variants": {"short": {"timestamp": "now"},
                                    "long": {"timestamp": "Sep 28, 9:41 PM"}}},
    "tool-cell": {"scene": "4", "source": "conversation-04", "bindings": ["kind", "summary", "detail",
                  "status", "expanded", "output"],
                  "variants": {"short": {"summary": "Read steer_queue.rs", "status": "ok"},
                               "long": {"summary": "Ran cargo test -p octos-cli", "status": "ok",
                                        "output": "running 12 tests\u2026 12 passed"}}},
    "composer": {"scene": "8", "source": "conversation-08", "bindings": ["draft", "model", "running", "queued",
                 "action:submit", "action:interrupt", "action:steer"],
                 "variants": {"short": {"model": "v4-flash"}, "long": {"model": "v4-flash", "queued": True}}},
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
    env = {"BEAUTY_PYTHON": PY}
    result = subprocess.run(argv, env={**os.environ, **env}, capture_output=True, text=True)
    tail = (result.stdout or result.stderr).strip().splitlines()[-2:]
    print("\n".join(tail))
    if result.returncode != 0:
        raise SystemExit("extract failed: " + (result.stderr or result.stdout)[-800:])


def main():
    run_extract()
    catalogue = json.loads((STAGE / "catalogue.json").read_text())
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)
    index = {"schema_version": 1, "kind": "octoscode-l0-components",
             "note": "Reusable per-item components compiled from the approved conversation scenes "
                     "(card #16). Width-responsive: fill the slot width, height from content.",
             "components": []}
    for card in catalogue["cards"]:
        cid = card["id"]
        spec = SPEC[cid]
        src = STAGE / card["folder"]
        dest = OUT / cid
        shutil.copytree(src, dest)
        # carry the renderable kit card under the component folder (Gate B input)
        entry = {"id": cid, "source_scene": spec["source"], "extracted_from": f"scene {card['scene']} "
                 f"root '{card['root']}'", "owner": card["owner"], "bindings": spec["bindings"],
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
