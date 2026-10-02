#!/usr/bin/env python3
"""Render the seven #D2b settings cards with beauty-host (the setup-board
render() verbatim: BEAUTY_REQUEST + --remote + /g grab + /quit), after
publishing each card's digest-named svg assets into the gallery docroot so
the compiled artwork URLs (http://127.0.0.1:8170/ux-images/settings-XX/...)
resolve. Native canvas: 812x1552 (the 406x776 logical artboard @2x, the
reference OCR scale)."""
import json, os, shutil, socket, subprocess, sys, threading, time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]          # repo root (tools->settings->stage-b->design->root)
SET = ROOT / "design/stage-b/settings"
FLOW = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow/flows/image-lib"
PUBLISHED = FLOW / "published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
import re as _re
PORT = None
for _c in (8170, 8188, 8189, 8190):
    import socket as _s
    _t = _s.socket()
    try:
        _t.bind(("127.0.0.1", _c)); PORT = _c; break
    except OSError:
        continue
    finally:
        _t.close()
assert PORT, "no free artwork port"

def free_port(cands):
    for c in cands:
        t = socket.socket()
        try:
            t.bind(("127.0.0.1", c))
            return c
        except OSError:
            continue
        finally:
            t.close()
    raise SystemExit("no free port")

def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5):
                return True
        except OSError:
            time.sleep(0.25)
    return False

# 1) publish the compiled assets into the gallery docroot + re-point the
# compiled artwork URLs at OUR port (8170 belongs to an unrelated process)
for k in range(6, 13):
    cid = f"settings-{k:02d}"
    d = SET / "cards" / cid
    card = d / "page.card"
    card.write_text(card.read_text().replace("127.0.0.1:8170", f"127.0.0.1:{PORT}"))
    # the svg src URLs live in the KIT PACK (compile puts them in the
    # component style -> kit.json), not in page.card — the 501s in host.log
    # were beauty-host loading the bridge process on 8170
    for extra in (d / "kit/native/light/kit.json", d / "page.data.json"):
        if extra.exists():
            extra.write_text(extra.read_text().replace("127.0.0.1:8170", f"127.0.0.1:{PORT}"))
    out = PUBLISHED / "ux-images" / cid / "assets"
    out.mkdir(parents=True, exist_ok=True)
    n = 0
    for a in sorted((d / "assets").glob("*.svg")):
        shutil.copy2(a, out / a.name)
        n += 1
    print(f"published {cid}: {n} assets")

# 2) the artwork server on 8170 (docroot = published/, so /ux-images/... resolves)
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass
httpd = ThreadingHTTPServer(("127.0.0.1", PORT), partial(Quiet, directory=str(PUBLISHED)))
threading.Thread(target=httpd.serve_forever, daemon=True).start()
time.sleep(0.5)
assert wait_port(PORT, 5), "artwork server did not come up"

# 3) render each card (setup render() verbatim shape)
results = {}
for k in range(6, 13):
    cid = f"settings-{k:02d}"
    d = SET / "cards" / cid
    port = free_port(list(range(8390, 8400)))
    request = {"card": str(d / "page.card"), "data": str(d / "page.data.json"),
               "kit_dir": str(d / "kit"), "format": "l0-kit", "width": 812, "height": 1552,
               "nonce": f"settings-{cid}-{k}", "result": str(d / "native.json"),
               "layout": str(d / "layout.json"), "actions": str(d / "actions.json")}
    (d / "request.json").write_text(json.dumps(request))
    log = (d / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1",
           "BEAUTY_REQUEST": str(d / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            results[cid] = "no port"
            continue
        time.sleep(5)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        proc.wait(timeout=15)
    finally:
        if proc.poll() is None:
            proc.kill()
        log.close()
    if png and Path(png).is_file():
        shutil.copy2(png, d / "native.png")
        results[cid] = f"native.png {Path(png).stat().st_size}B"
    else:
        results[cid] = f"no grab (host.log tail: {(d/'host.log').read_text()[-160:]!r})"
print(json.dumps(results, indent=1))
