#!/usr/bin/env python3
"""#29b — live-data headless capture of the three wired board-2 screens.

Renders setup-04/05/06 from tmp copies whose mapped trees carry the values the
wired screen shows after `ws::refresh` against the recorded r2-profile run
(`/tmp/ws29b` root, its crates/design/docs/harness entries, the dsflash
permission/model state) — the same render path Stage B used (the flow's own
`compile_page` + a hidden beauty-host), so the PNGs prove the bindings feed the
card's data slots. Nothing under design/ is touched.

Outputs: docs/cards/29b-live-*.png + 29b-live-bindings.json (machine-readable).
Run: python3 docs/cards/29b_live_capture.py
"""
import json
import os
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
CARDS = ROOT / "design/stage-b/setup/cards"
WORK = ROOT / "tmp/29b-live"
ART_PORT = 8182
PORTS = [8391, 8392, 8393]
ART = "http://127.0.0.1:8182"

sys.path.insert(0, str(CLONE / "flows/image-lib"))
from compile import compile_page  # noqa: E402

# The r2-profile recorded state (crates/octoscode-client/tests/fixtures/
# r2-profile-a6ea8505.jsonl) — what ws::refresh leaves in the screen cache.
LIVE = {
    "setup-04": {
        "folder_field": {"text": "/tmp/ws29b"},
        # #29b2: the title node stays "Server folder" (the binding, not the
        # capture, now decides it); only the field carries the path.
        "t_ws0_name": {"text": "crates"}, "t_ws0_path": {"text": "/tmp/ws29b/crates"},
        "t_ws1_name": {"text": "design"}, "t_ws1_path": {"text": "/tmp/ws29b/design"},
        "t_ws2_name": {"text": "docs"}, "t_ws2_path": {"text": "/tmp/ws29b/docs"},
        "t_ws3_name": {"text": "harness"}, "t_ws3_path": {"text": "/tmp/ws29b/harness"},
    },
    "setup-05": {
        "model_field": {"text": "deepseek-v4-flash"},
        "t_sb_r": {"text": "· Network allowed · workspace"},
        "t_sb_b": {"text": "Enabled"},
    },
    "setup-06": {
        "t_g1_v0": {"text": "Live"},
        # #29b2: the Workspace row shows the NAME (basename), not the path.
        "t_g1_v1": {"text": "ws29b"},
        "t_g1_v2": {"text": "dsflash"},
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
        "width": 406, "height": 776, "nonce": f"29b-live-{port}",
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
            ("setup-04", "29b-live-workspace-picker.png"),
            ("setup-05", "29b-live-session-settings.png"),
            ("setup-06", "29b-live-general-settings.png"),
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
    (HERE / "29b-live-bindings.json").write_text(json.dumps({
        "schema_version": 1, "card": "29b",
        "source_fixture": "crates/octoscode-client/tests/fixtures/r2-profile-a6ea8505.jsonl",
        "bindings": ws_ids()["bindings"],
        "actions": ws_ids()["actions"],
        "captures": summary,
    }, indent=2) + "\n")
    print("LIVE_CAPTURE_DONE")


def ws_ids():
    """The board-2 id table straight from the compiled module (no drift)."""
    ids = {"bindings": [b[0] for b in ws_table("BINDINGS")],
           "actions": [a[0] for a in ws_table("ACTIONS")]}
    return ids


def ws_table(name):
    src = (ROOT / "crates/octoscode-module/src/screens/workspace.rs").read_text()
    import re
    block = re.search(rf"pub const {name}: &\[\(&str, &str\)\] = &\[(.*?)\];", src, re.S).group(1)
    return re.findall(r'\("([^"]+)"', block)


if __name__ == "__main__":
    main()
