#!/usr/bin/env python3
"""Gate-B round 1 for the six remaining scenes (05/06/07/10/11/12).

Same mechanism as render_v4.py (the card #11f host), on this card's port block
8340-8345. Renders each scene hidden (MAKEPAD_HIDE_WINDOWS=1), writes the snap,
the native PNG, and a side-by-side review PNG (atlas crop | native) so the
geometry can be compared without a display.
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
ROOT = Path(__file__).resolve().parents[4]          # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
EVIDENCE = HERE / "evidence/gate-b"
WORK = ROOT / "tmp/stage-b/render-18"
ART_PORT = 8180
SCENES = [("05", 8340), ("06", 8341), ("07", 8342), ("10", 8343), ("11", 8344), ("12", 8345)]


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def serve_published():
    handler = partial(Quiet, directory=str(PUBLISHED))
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd


def wait_port(port, timeout=40):
    import socket
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False


def render(n, port, round_tag):
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
        "format": "l0-kit", "width": 406, "height": 776, "nonce": f"gate-b-18-{n}-{round_tag}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env, stdout=log,
                            stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            return {"scene": n, "error": "no port"}
        time.sleep(6)
        snap = subprocess.check_output(
            ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
        (EVIDENCE / f"conversation-{n}-snap-{round_tag}.json").write_text(snap)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        if png and Path(png).is_file():
            shutil.copy(png, EVIDENCE / f"conversation-{n}-native-{round_tag}.png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    text = (work / "host.log").read_text(errors="ignore")
    return {
        "scene": n, "port": port, "png": png,
        "font_warnings": text.count("not available in this build"),
        "widgets": len(json.loads((EVIDENCE / f"conversation-{n}-snap-{round_tag}.json").read_text())["s"])
        if (EVIDENCE / f"conversation-{n}-snap-{round_tag}.json").exists() else None,
    }


def side_by_side(n, round_tag):
    """atlas crop (left) | native render (right), both 406 wide."""
    from PIL import Image
    ref = Image.open(HERE / "cards" / f"conversation-{n}" / "reference.png").convert("RGB")
    ref = ref.resize((406, 776), Image.LANCZOS)
    nat_path = EVIDENCE / f"conversation-{n}-native-{round_tag}.png"
    if not nat_path.exists():
        return None
    nat = Image.open(nat_path).convert("RGB")
    if nat.size != (406, 776):
        nat = nat.resize((406, 776), Image.LANCZOS)
    canvas = Image.new("RGB", (406 * 2 + 12, 776), (255, 255, 255))
    canvas.paste(ref, (0, 0))
    canvas.paste(nat, (406 + 12, 0))
    out = EVIDENCE / f"conversation-{n}-review-{round_tag}.png"
    canvas.save(out)
    return out


if __name__ == "__main__":
    import sys
    round_tag = sys.argv[1] if len(sys.argv) > 1 else "v1"
    only = sys.argv[2].split(",") if len(sys.argv) > 2 else None
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    httpd = serve_published()
    time.sleep(1)
    try:
        for n, port in SCENES:
            if only and n not in only:
                continue
            r = render(n, port, round_tag)
            sb = side_by_side(n, round_tag) if r.get("png") else None
            print(json.dumps({**r, "review": str(sb) if sb else None}))
    finally:
        httpd.shutdown()
