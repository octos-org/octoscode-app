#!/usr/bin/env python3
"""Render short/long variants for the two autonomy components (card #28b, step 2):
task-card and attachment-card, hidden, at 360 and 540 wide.

Simplified mirror of board 1's render_variants.py: copies the component, applies a
text-only variant transform to its mapped.json tree, re-compiles with the flow's own
compile_page, renders hidden with beauty-host, writes variants/<variant>-<w>.png and
review-<variant>.png (reference | native@360 | native@540), and checks the
rightmost-4px background rule.
"""
import json
import os
import shutil
import subprocess
import threading
import time
from collections import Counter
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parents[1]          # design/stage-b/autonomy
ROOT = Path(__file__).resolve().parents[4]          # repo root
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
COMPONENTS = ROOT / "design/components"
WORK = ROOT / "tmp/stage-b/render-variants-28b"
ART_PORT = 8183
PORTS = [8356, 8357]
WIDTHS = (360, 540)

# text-only variants keyed by node id (task-card: command text; attachment-card:
# the progress ring is att_2-only, so short = bare card, long = with ring+pct)
VARIANTS = {
    "task-card": {
        "short": {"t_cmd": "› cargo clippy -p octos-cli"},
        "long": {"t_cmd": "› cargo test -p octos-cli steer_queue -- --nocapture"},
    },
    "attachment-card": {
        "short": {"att_1_img": None},   # bare card, icon only
        "long": {"att_1_img": None},
    },
}


class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


def serve_published():
    PUBLISHED.mkdir(parents=True, exist_ok=True)
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


def walk(n):
    yield n
    for c in n.get("c", []):
        yield from walk(c)


def apply_variant(mapped_path, comp, variant):
    doc = json.loads(mapped_path.read_text())
    texts = VARIANTS[comp][variant]
    for n in walk(doc["tree"]):
        if n["id"] in texts and texts[n["id"]] is not None:
            n["text"] = texts[n["id"]]
    # Card #18c/#18e (render_variants.py:705-713): the extracted root is card-tight,
    # so at 360 wide it runs edge-to-edge and the rightmost 4px are card fill, not
    # page background. Give the root the page gutter the atlas normalised away —
    # and move the WHOLE subtree with it (children carry absolute coords).
    tree = doc["tree"]
    dx = 16 - (tree.get("x") or 0)
    dy = 8 - (tree.get("y") or 0)
    for n in walk(tree):
        if n.get("x") is not None:
            n["x"] = round(n["x"] + dx, 2)
        if n.get("y") is not None:
            n["y"] = round(n["y"] + dy, 2)
    tree["padright"] = 16.0
    # measure/map re-fits the root width to its atlas value (360), which after the
    # 16px gutter shift overflows the 360 slot (right = x+w = 376). Clamp the root
    # width so right edge + gutter <= 360; children are relative to the root box.
    max_right = 360 - 16
    if (tree.get("x") or 0) + (tree.get("w") or 0) > max_right:
        tree["w"] = round(max_right - tree["x"], 2)
    mapped_path.write_text(json.dumps(doc, indent=2) + "\n")


def rightmost_bg(png):
    """The rightmost 4 px must be page background (white or host ground), not card fill."""
    a = np.asarray(Image.open(png).convert("RGB"))
    strip = a[:, -4:, :]
    white = bool((strip.min(axis=2) > 238).all())
    ground = bool((np.abs(strip - 76).max(axis=2) <= 8).all())
    top = Counter(map(tuple, strip.reshape(-1, 3))).most_common(1)[0][0]
    return white or ground, [int(v) for v in top]


def render_variant(comp, variant, work, port):
    compile_cmd = (
        f"import sys;sys.path.insert(0,'{CLONE}/flows/image-lib');sys.path.insert(0,'{CLONE}/flows');"
        f"from compile import compile_page;compile_page('{work}')")
    subprocess.run(["python3", "-c", compile_cmd], check=True, capture_output=True)
    data = work / "page.data.json"
    data.write_text(data.read_text().replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
    results = {}
    for w in WIDTHS:
        request = {
            "card": str(work / "page.card"), "data": str(data), "kit_dir": str(work / "kit"),
            "format": "l0-kit", "width": w, "height": 776, "nonce": f"28b-{comp}-{variant}-{w}",
            "result": str(work / f"native-{w}.json"), "layout": str(work / f"layout-{w}.json"),
            "actions": str(work / f"actions-{w}.json"),
        }
        (work / "request.json").write_text(json.dumps(request))
        log = (work / f"host-{w}.log").open("w")
        env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
        proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env, stdout=log,
                                stderr=subprocess.STDOUT)
        try:
            if not wait_port(port):
                results[w] = None
                continue
            time.sleep(5)
            grab = json.loads(subprocess.check_output(
                ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
            png = grab.get("png")
            out = COMPONENTS / comp / "variants"
            out.mkdir(exist_ok=True)
            if png and Path(png).is_file():
                dest = out / f"{variant}-{w}.png"
                shutil.copy(png, dest)
                results[w] = dest
            subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                           capture_output=True)
            time.sleep(1)
        finally:
            if proc.poll() is None:
                proc.kill()
            log.close()
    return results


def review(comp, variant):
    ref = Image.open(COMPONENTS / comp / "reference.png").convert("RGB")
    ref.thumbnail((360, 776), Image.LANCZOS)
    imgs = []
    for w in WIDTHS:
        p = COMPONENTS / comp / "variants" / f"{variant}-{w}.png"
        if p.exists():
            im = Image.open(p).convert("RGB")
            im.thumbnail((w, 776), Image.LANCZOS)
            imgs.append(im)
    if not imgs:
        return None
    H = max([ref.height] + [i.height for i in imgs])
    canvas = Image.new("RGB", (ref.width + sum(i.width for i in imgs) + 24, H), (255, 255, 255))
    x = 0
    canvas.paste(ref, (x, 0)); x += ref.width + 12
    for im in imgs:
        canvas.paste(im, (x, 0)); x += im.width + 12
    out = COMPONENTS / comp / f"review-{variant}.png"
    canvas.save(out)
    return out


def main():
    httpd = serve_published()
    time.sleep(1)
    try:
        for i, comp in enumerate(VARIANTS):
            for variant in VARIANTS[comp]:
                work = WORK / f"{comp}-{variant}"
                if work.exists():
                    shutil.rmtree(work)
                work.mkdir(parents=True)
                src = COMPONENTS / comp
                for f in ("contract.json", "generation.json", "image-prompt.md",
                          "mapping.json", "page.card", "page.data.json", "semantic-map.json",
                          "service-actions.json", "reference.png"):
                    shutil.copy(src / f, work / f)
                shutil.copytree(src / "kit", work / "kit")
                if (src / "assets").is_dir():
                    shutil.copytree(src / "assets", work / "assets")
                # variant transform on the mapped tree, then compile from it
                shutil.copy(src / "mapped.json", work / "mapped.json")
                apply_variant(work / "mapped.json", comp, variant)
                renders = render_variant(comp, variant, work, PORTS[i % len(PORTS)])
                rv = review(comp, variant)
                checks = {w: rightmost_bg(p) for w, p in renders.items()}
                print(json.dumps({"component": comp, "variant": variant,
                                  "renders": {str(w): str(p) for w, p in renders.items()},
                                  "rightmost_bg": {str(w): v for w, v in checks.items()},
                                  "review": str(rv)}))
    finally:
        httpd.shutdown()


if __name__ == "__main__":
    main()
