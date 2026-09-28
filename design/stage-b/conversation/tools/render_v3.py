#!/usr/bin/env python3
"""Render the 5 gate-B components with the flow's own host (beauty-host), hidden.

Self-contained: serves the compiled artwork from an in-process loopback HTTP
server (daemon thread), so there is no background process to leak. One fresh
beauty-host per scene on our own port block, `MAKEPAD_HIDE_WINDOWS=1`.

Ports: 8391..8396 (p0-harness / card #5 block is 8300-8309; 8390-8399 is
"manual / ad-hoc" in docs/harness/GUIDE.md).
"""
import json
import os
import shutil
import subprocess
import threading
import time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/conversation
ROOT = Path(__file__).resolve().parents[4]          # repo root (p0-harness)
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-target/release/beauty-host"
EVIDENCE = HERE / "evidence/gate-b"
WORK = ROOT / "tmp/stage-b/render-v3"
ART_PORT = 8179
SCENES = [("01", 8391), ("03", 8393), ("04", 8394), ("08", 8395), ("09", 8396)]


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def serve_published():
    handler = partial(Quiet, directory=str(PUBLISHED))
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd


def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with __import__("socket").create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def render(n, port):
    work = WORK / f"conversation-{n}"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    src = HERE / "cards" / f"conversation-{n}"
    for f in ("page.card", "page.data.json"):
        shutil.copy(src / f, work / f)
    shutil.copytree(src / "kit", work / "kit")
    data = work / "page.data.json"
    data.write_text(data.read_text().replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
    request = {
        "card": str(work / "page.card"), "data": str(data), "kit_dir": str(work / "kit"),
        "format": "l0-kit", "width": 406, "height": 776, "nonce": f"gate-b-v3-{n}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        if not wait_port(port):
            return {"scene": n, "error": "no port"}
        time.sleep(6)                                   # let the scene mount and the SVGs resolve
        snap = subprocess.check_output(
            ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
        (EVIDENCE / f"conversation-{n}-snap-v3.json").write_text(snap)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        if png and Path(png).is_file():
            shutil.copy(png, EVIDENCE / f"conversation-{n}-native-v3.png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    text = (work / "host.log").read_text(errors="ignore")
    splash = work / "layout.json.native.splash"
    return {
        "scene": n, "port": port, "png": png,
        "font_warnings": text.count("not available in this build"),
        "border_widths": (splash.read_text().count("draw_bg.border_width: 1") if splash.exists() else None),
        "widgets": len(json.loads((EVIDENCE / f"conversation-{n}-snap-v3.json").read_text())["s"]),
    }


if __name__ == "__main__":
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    httpd = serve_published()
    time.sleep(1)
    try:
        for n, port in SCENES:
            print(json.dumps(render(n, port)))
    finally:
        httpd.shutdown()
