#!/usr/bin/env python3
"""Render the nine #D1 phase4 scenes with the flow's own host (beauty-host), hidden.

Adapted from setup/tools/render_setup.py (board 2): serves the clone's published
artwork from an in-process loopback server, one fresh beauty-host per scene on
THIS lane's ports (8350..8358, p0-map-e block; the artwork server takes 8359),
MAKEPAD_HIDE_WINDOWS=1, then composes the Gate-B review PNG (reference | native
render side by side) into design/stage-b/phase4/evidence/gate-b/. Self-contained.
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

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/phase4
ROOT = Path(__file__).resolve().parents[4]          # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
EVIDENCE = HERE / "evidence/gate-b"
WORK = ROOT / "tmp/stage-b/render-phase4"
ART_PORT = 8359
SCENES = [("01", 8350), ("02", 8351), ("03", 8352), ("04", 8353), ("05", 8354),
          ("06", 8355), ("07", 8356), ("08", 8357), ("09", 8358)]


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


def compose_review(n, native_png, out):
    """reference | native render, side by side, labelled."""
    ref = Image.open(HERE / "cards" / f"p4-{n}" / "reference.png").convert("RGB")
    ref = ref.resize((406, 776))
    nat = Image.open(native_png).convert("RGB")
    if nat.size != (406, 776):
        nat = nat.resize((406, 776))
    pad, gutter = 12, 8
    W = pad * 2 + 406 * 2 + gutter
    H = pad * 2 + 776 + 22
    sheet = Image.new("RGB", (W, H), (255, 255, 255))
    d = ImageDraw.Draw(sheet)
    sheet.paste(ref, (pad, pad + 22))
    sheet.paste(nat, (pad + 406 + gutter, pad + 22))
    d.text((pad, 6), f"p4-{n}  reference (target)", fill=(20, 20, 20))
    d.text((pad + 406 + gutter, 6), "native render", fill=(20, 20, 20))
    sheet.save(out)


def render(n, port):
    work = WORK / f"p4-{n}"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    src = HERE / "cards" / f"p4-{n}"
    for f in ("page.card", "page.data.json"):
        shutil.copy(src / f, work / f)
    shutil.copytree(src / "kit", work / "kit")
    data = work / "page.data.json"
    body = data.read_text()
    if "127.0.0.1:8170" in body:                       # artwork prefix -> our server
        data.write_text(body.replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
    request = {
        "card": str(work / "page.card"), "data": str(data), "kit_dir": str(work / "kit"),
        "format": "l0-kit", "width": 406, "height": 776, "nonce": f"gate-b-p4-v1-{n}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env, stdout=log, stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            return {"scene": n, "error": "no port"}
        time.sleep(6)                                   # let the scene mount and the SVGs resolve
        snap = subprocess.check_output(
            ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
        (EVIDENCE / f"p4-{n}-snap-v1.json").write_text(snap)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        if png and Path(png).is_file():
            shutil.copy(png, EVIDENCE / f"p4-{n}-native-v1.png")
            compose_review(n, EVIDENCE / f"p4-{n}-native-v1.png",
                           EVIDENCE / f"p4-{n}-review-v1.png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/gq"],
                       capture_output=True)
        time.sleep(1)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    text = (work / "host.log").read_text(errors="ignore")
    widgets = None
    try:
        widgets = len(json.loads((EVIDENCE / f"p4-{n}-snap-v1.json").read_text())["s"])
    except Exception:
        pass
    return {
        "scene": n, "port": port, "png": str(png),
        "font_warnings": text.count("not available in this build"),
        "widgets": widgets,
    }


if __name__ == "__main__":
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    httpd = serve_published()
    time.sleep(1)
    results = []
    try:
        for n, port in SCENES:
            r = render(n, port)
            results.append(r)
            print(json.dumps(r), flush=True)
    finally:
        httpd.shutdown()
    (EVIDENCE / "render-v1-summary.json").write_text(json.dumps(results, indent=2) + "\n")
    print("RENDER_PHASE4_DONE")
