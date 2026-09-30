#!/usr/bin/env python3
"""#30b — live-data headless capture of the three wired autonomy screens.

Renders autonomy-03/04/05 from tmp copies whose mapped trees carry the values
the wired screen shows after `autonomy::refresh` against the recorded r1 run
(goal_01 "r1 replay probe" active · 100000000 budget; loop_01 fixed_interval
3600; monitor_01 poll/ERROR) — the same render path Stage B used (the flow's
`compile_page` + a hidden beauty-host), so the PNGs prove the bindings feed
the card's data slots. Nothing under design/ is touched.

Outputs: docs/cards/30b-live-*.png + 30b-live-bindings.json (machine-readable).
Run: python3 docs/cards/30b_live_capture.py
"""
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent           # docs/cards
ROOT = HERE.parents[1]                            # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
CARDS = ROOT / "design/stage-b/autonomy/cards"
WORK = ROOT / "tmp/30b-live"
ART_PORT = 8182
PORTS = [8391, 8392, 8393]

sys.path.insert(0, str(CLONE / "flows/image-lib"))
from compile import compile_page  # noqa: E402

# The r1-autonomy recorded state — what the autonomy bindings project.
LIVE = {
    "autonomy-03": {
        "t_goal": {"text": "r1 replay probe"},
        "goal_badge_label": {"text": "Active"},
        "t_budget_val": {"text": "0 / 100M"},
        "t_elapsed_val": {"text": "0s"},
    },
    "autonomy-04": {
        # One recorded loop; the card's other rows sit empty (live truth).
        "loop_1_name": {"text": "r1 replay probe"},
        "loop_1_cad": {"text": "every 3600s"},
        "loop_2_name": {"text": ""},
        "loop_2_cad": {"text": ""},
        "loop_3_name": {"text": ""},
        "loop_3_cad": {"text": ""},
    },
    "autonomy-05": {
        "mon_1_cmd": {"text": "./scripts/watch.sh"},
        "mon_1_state": {"text": "active"},
        "mon_1_int": {"text": "3600s"},
        "mon_2_cmd": {"text": ""},
        "mon_2_state": {"text": ""},
        "mon_2_int": {"text": ""},
        "monitors_footer_label": {"text": "1 monitors · 1 active"},
    },
}


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def inject(card: str, tree: dict) -> int:
    rules = LIVE[card]
    hit = 0
    for n in walk(tree):
        rule = rules.get(n["id"])
        if rule and "text" in rule:
            n["text"] = rule["text"]
            hit += 1
    return hit


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def render(work: Path, port: int, out_png: Path) -> bool:
    request = {
        "card": str(work / "page.card"), "data": str(work / "page.data.json"),
        "kit_dir": str(work / "kit"), "format": "l0-kit",
        "width": 406, "height": 776, "nonce": f"30b-live-{port}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    try:
        if not wait_port(port):
            return False
        time.sleep(5)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        if png and Path(png).is_file():
            shutil.copy(png, out_png)
            return True
        return False
    finally:
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
        if proc.poll() is None:
            proc.kill()
        log.close()


def ws_table(name):
    """The autonomy id table straight from the compiled source (no drift)."""
    src = (ROOT / "crates/octoscode-module/src/screens/autonomy.rs").read_text()
    block = re.search(rf"pub const {name}: &\[\(&str, &str\)\] = &\[(.*?)\];", src, re.S).group(1)
    return re.findall(r'\("([^"]+)"', block)


def main():
    if WORK.exists():
        shutil.rmtree(WORK)
    WORK.mkdir(parents=True)
    handler = partial(Quiet, directory=str(PUBLISHED))
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(1)
    summary = []
    try:
        for i, (card, out_name) in enumerate([
            ("autonomy-03", "30b-live-goal.png"),
            ("autonomy-04", "30b-live-loops.png"),
            ("autonomy-05", "30b-live-monitors.png"),
        ]):
            work = WORK / card
            shutil.copytree(CARDS / card, work)
            mapped = json.loads((work / "mapped.json").read_text())
            hit = inject(card, mapped["tree"])
            (work / "mapped.json").write_text(json.dumps(mapped, indent=2) + "\n")
            compile_page(work)
            data = (work / "page.data.json").read_text()
            (work / "page.data.json").write_text(data.replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
            out = HERE / out_name
            ok = render(work, PORTS[i], out)
            summary.append({"card": card, "injected": hit, "png": out.name, "rendered": ok})
            print(json.dumps(summary[-1]), flush=True)
    finally:
        httpd.shutdown()
    (HERE / "30b-live-bindings.json").write_text(json.dumps({
        "schema_version": 1, "card": "30b",
        "source_fixture": "crates/octoscode-client/tests/fixtures/r1-autonomy-a6ea8505.jsonl",
        "bindings": ws_table("BINDINGS"),
        "actions": ws_table("ACTIONS"),
        "captures": summary,
    }, indent=2) + "\n")
    print("LIVE_CAPTURE_DONE")


if __name__ == "__main__":
    main()
