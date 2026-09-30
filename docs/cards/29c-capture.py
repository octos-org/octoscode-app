#!/usr/bin/env python3
"""Entry #29c visual capture: render the three LIVE screen cards.

Each card under target/f29c-live/<screen>/ was written by the f29c
`live_capture_writes_the_three_cards_with_store_values` test: the recorded
r2-profile frames folded into the store, then `lower_card_src` injected the
store values into the authored `copy` slots. This driver renders those cards
with the SAME beauty-host chain the Gate-B renders used (l0-kit, 406x776),
grabs /g, and composes reference|live review sheets into
design/stage-b/setup/evidence/gate-b/.

Usage: python3 docs/cards/29c-capture.py
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

ROOT = Path(__file__).resolve().parents[2]
WS = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = WS / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
LIVE = ROOT / "target/f29c-live"
EVIDENCE = ROOT / "design/stage-b/setup/evidence/gate-b"
SCREENS = ["setup-07", "setup-09", "setup-10"]


def free_port(cands):
    for c in cands:
        s = socket.socket()
        try:
            s.bind(("127.0.0.1", c))
            return c
        except OSError:
            continue
        finally:
            s.close()
    raise SystemExit("no free port")


def wait_port(port, timeout=40):
    for _ in range(timeout * 4):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def render(work, port, art_port):
    data = work / "page.data.json"
    body = data.read_text()
    if "127.0.0.1:8170" in body:            # artwork prefix -> our server
        data.write_text(body.replace("127.0.0.1:8170", f"127.0.0.1:{art_port}"))
    request = {
        "card": str(work / "page.card"), "data": str(data),
        "kit_dir": str(work / "kit"), "format": "l0-kit",
        "width": 406, "height": 776, "nonce": f"29c-live-{work.name}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1",
           "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            return None, "no port"
        time.sleep(12)   # 6s left the four setup-07 SVG fetches unpainted
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    if png and not Path(png).resolve().is_relative_to(ROOT):
        return None, "foreign host answered"
    return png, None


def main():
    art_port = free_port([8195, 8196, 8197])
    class Quiet(SimpleHTTPRequestHandler):
        def log_message(self, *a):
            pass
    httpd = ThreadingHTTPServer(("127.0.0.1", art_port),
                                partial(Quiet, directory=str(PUBLISHED)))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(0.5)
    ports = [free_port([8395, 8396]), free_port([8397, 8398])]
    summary = {}
    try:
        for i, screen in enumerate(SCREENS):
            work = LIVE / screen
            if not (work / "page.card").is_file():
                summary[screen] = {"error": "run the f29c test first"}
                continue
            png, err = render(work, ports[i % 2], art_port)
            if not png:
                summary[screen] = {"error": err}
                continue
            native = EVIDENCE / f"{screen}-native-live-29c.png"
            shutil.copy(png, native)
            ref = ROOT / f"design/stage-b/setup/cards/{screen}/reference.png"
            from PIL import Image
            a = Image.open(ref).convert("RGB")
            b = Image.open(native).convert("RGB")
            if b.size != a.size:
                b = b.resize(a.size)
            w, h = a.size
            sheet = Image.new("RGB", (w * 2 + 10, h), (255, 0, 255))
            sheet.paste(a, (0, 0))
            sheet.paste(b, (w + 10, 0))
            sheet.save(EVIDENCE / f"{screen}-review-live-29c.png")
            summary[screen] = {"png": str(native), "review": str(EVIDENCE / f"{screen}-review-live-29c.png")}
            print(screen, "->", summary[screen]["review"], flush=True)
    finally:
        httpd.shutdown()
    (EVIDENCE / "capture-live-29c.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    sys.exit(main())
