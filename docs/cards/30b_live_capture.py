#!/usr/bin/env python3
"""#30b4 — capture the board-3 screens through the APP's OWN path.

Drives `examples/screen_shot_autonomy` (the feature-flagged temporary mount):
the host lowers each wired card through the PRODUCTION
`screens::autonomy::lower_screen` (per-item rows, card-height-from-rows,
body-font empty slots, goal.fill bar — all decided there, #30b4) and mounts it
with the module's MountCache. No capture-side tree edits: what the PNG shows
is what the app renders.

    OCTOSCODE_SCREEN=goal|loops|monitors  OCTOSCODE_STORE=0|1|3
    harness/headless.sh start|shot|stop   (hidden window, my port block)

Outputs: docs/cards/30b-live-*.png + 30b-live-bindings.json (machine-readable).
Run: python3 docs/cards/30b_live_capture.py
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent           # docs/cards
ROOT = HERE.parents[1]                            # repo root
BIN = ROOT / "target/debug/examples/screen_shot_autonomy"
HEADLESS = ROOT / "harness/headless.sh"
# goal + loops x3 + monitors x3; the host's asset server sits at 8394.
ENTRIES = [
    ("goal", 1, "goal"),
    ("loops", 0, "loops-0"),
    ("loops", 1, "loops-1"),
    ("loops", 3, "loops-3"),
    ("monitors", 0, "monitors-0"),
    ("monitors", 1, "monitors-1"),
    ("monitors", 3, "monitors-3"),
]
PORTS = list(range(8385, 8392))


def sh(args, env=None, timeout=120):
    return subprocess.run(args, env=env, capture_output=True, text=True, timeout=timeout)


def capture(screen: str, n: int, name: str, port: int, out: Path) -> bool:
    env = {
        **os.environ,
        "OCTOSCODE_SCREEN": screen,
        "OCTOSCODE_STORE": str(n),
        "MAKEPAD_HIDE_WINDOWS": "1",
    }
    started = sh(["bash", str(HEADLESS), "start", str(BIN), str(port)], env=env, timeout=180)
    if started.returncode != 0:
        print(started.stdout[-300:], started.stderr[-300:], flush=True)
        return False
    try:
        # The host mounts on its first draw; give the splash a beat.
        sh(["sleep", "4"])
        shot = sh(["bash", str(HEADLESS), "shot", str(port), str(out)], timeout=90)
        if shot.returncode != 0 or not out.is_file():
            print(shot.stdout[-300:], shot.stderr[-300:], flush=True)
            return False
        return True
    finally:
        sh(["bash", str(HEADLESS), "stop", str(port)], timeout=60)


def ws_table(name):
    """The autonomy id table straight from the compiled source (no drift)."""
    src = (ROOT / "crates/octoscode-module/src/screens/autonomy.rs").read_text()
    block = re.search(rf"pub const {name}: &\[\(&str, &str\)\] = &\[(.*?)\];", src, re.S).group(1)
    return re.findall(r'\("([^"]+)"', block)


def main():
    if not BIN.is_file():
        sys.exit(f"missing {BIN}; build the example first")
    summary = []
    for (screen, n, name), port in zip(ENTRIES, PORTS):
        out = HERE / f"30b-live-{name}.png"
        ok = capture(screen, n, name, port, out)
        summary.append({"screen": screen, "store": n, "png": out.name, "rendered": ok})
        print(json.dumps(summary[-1]), flush=True)
    (HERE / "30b-live-bindings.json").write_text(json.dumps({
        "schema_version": 1, "card": "30b4",
        "host": "examples/screen_shot_autonomy.rs (the production lower_screen)",
        "source_fixture": "crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl",
        "bindings": ws_table("BINDINGS"),
        "actions": ws_table("ACTIONS"),
        "captures": summary,
    }, indent=2) + "\n")
    print("LIVE_CAPTURE_DONE")


if __name__ == "__main__":
    main()
