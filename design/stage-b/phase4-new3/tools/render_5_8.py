#!/usr/bin/env python3
"""D3b — render board-3 screens 5-8 with the flow's own host (beauty-host),
hidden. Lane-suffixed sibling of setup/tools/render_setup.py (same machine,
this lane's port block): serves the clone's published artwork in-process
(8384), one fresh beauty-host per card on 8380..8383, MAKEPAD_HIDE_WINDOWS=1,
then composes the Gate-B review PNG (atlas crop | native render) into
design/stage-b/phase4-new3/evidence/gate-b/.
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

HERE = Path(__file__).resolve().parent.parent      # design/stage-b/phase4-new3
ROOT = Path(__file__).resolve().parents[4]         # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
EVIDENCE = HERE / "evidence/gate-b"
WORK = ROOT / "tmp/stage-b/render-p4n3"
ART_PORT = 8384
# The pipeline compiled these cards with ARTWORK = http://127.0.0.1:8190/…
CARD_ART_PREFIX = "127.0.0.1:8190"
SCENES = [("05", 8380), ("06", 8381), ("07", 8382), ("08", 8383)]


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
    """atlas crop | native render, side by side, labelled."""
    ref = Image.open(HERE / "cards" / f"phase4n3-{n}" / "reference.png").convert("RGB")
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
    d.text((pad, 6), f"phase4n3-{n}  atlas crop (target)", fill=(20, 20, 20))
    d.text((pad + 406 + gutter, 6), "native render", fill=(20, 20, 20))
    sheet.save(out)


def render(n, port):
    work = WORK / f"phase4n3-{n}"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    src = HERE / "cards" / f"phase4n3-{n}"
    for f in ("page.card", "page.data.json"):
        shutil.copy(src / f, work / f)
    shutil.copytree(src / "kit", work / "kit")
    data = work / "page.data.json"
    body = data.read_text()
    if CARD_ART_PREFIX in body:                       # artwork prefix -> ours
        data.write_text(body.replace(CARD_ART_PREFIX, f"127.0.0.1:{ART_PORT}"))
    request = {
        "card": str(work / "page.card"), "data": str(data),
        "kit_dir": str(work / "kit"),
        "format": "l0-kit", "width": 406, "height": 776,
        "nonce": f"gate-b-p4n3-v4-{n}",
        "result": str(work / "native.json"), "layout": str(work / "layout.json"),
        "actions": str(work / "actions.json"),
    }
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1",
           "BEAUTY_REQUEST": str(work / "request.json")}
    # The port must be OURS before launch: an occupied port makes wait_port
    # hand back someone else's window (06 grabbed an old octosense, widgets=213).
    occupied = subprocess.run(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"],
                              capture_output=True).stdout
    if occupied:
        return {"scene": n, "error": f"port {port} occupied"}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            return {"scene": n, "error": "no port"}
        time.sleep(6)                                 # mount + svg resolve
        snap = subprocess.check_output(
            ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
        (EVIDENCE / f"p4n3-{n}-snap-v4.json").write_text(snap)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        # Only OUR beauty-host's grab is evidence (the v1 06 grab came from
        # a foreign window's /g).
        if png and "beauty-host" in png and Path(png).is_file():
            shutil.copy(png, EVIDENCE / f"p4n3-{n}-native-v4.png")
            compose_review(n, EVIDENCE / f"p4n3-{n}-native-v4.png",
                           EVIDENCE / f"p4n3-{n}-review-v4.png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    text = (work / "host.log").read_text(errors="ignore")
    widgets = None
    try:
        widgets = len(json.loads((EVIDENCE / f"p4n3-{n}-snap-v4.json").read_text())["s"])
    except Exception:
        pass
    return {"scene": n, "port": port, "png": str(png),
            "font_warnings": text.count("not available in this build"),
            "widgets": widgets}


if __name__ == "__main__":
    assert BEAUTY.is_file(), f"beauty-host missing: {BEAUTY}"
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    httpd = serve_published()
    time.sleep(1)
    results = []
    try:
        for n, port in SCENES:
            r = render(n, port)
            results.append(r)
            print(json.dumps(r))
    finally:
        httpd.shutdown()
    ok = sum(1 for r in results if r.get("png"))
    print(f"rendered {ok}/{len(SCENES)} -> {EVIDENCE}")
