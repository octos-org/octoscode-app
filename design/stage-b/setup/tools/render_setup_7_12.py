#!/usr/bin/env python3
"""Render the six setup cards with beauty-host (hidden), board-1 render_v4 pattern.

Differences from render_v4.py: project dir is design/stage-b/setup, evidence under
design/stage-b/setup/evidence/gate-b, and the artwork port is probed (8170, then
8185+) because other lanes hold fixed artwork ports on this shared host.
"""
import json, os, shutil, socket, subprocess, sys, threading, time
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parents[1]
ROOT = Path(__file__).resolve().parents[4]
CLONE = ROOT / "tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PUBLISHED = CLONE / "flows/image-lib/published"
BEAUTY = ROOT / "tmp/beauty-clone-target/release/beauty-host"
EVIDENCE = HERE / "evidence/gate-b"
WORK = ROOT / "tmp/stage-b/render-setup"
ART_PORT = None
for cand in (8170, 8185, 8186, 8187):
    s = socket.socket()
    try:
        s.bind(("127.0.0.1", cand)); s.close(); ART_PORT = cand; break
    except OSError:
        s.close()
assert ART_PORT, "no free artwork port"

class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *a): pass

def wait_port(port, timeout=40):
    for _ in range(int(timeout * 4)):
        try:
            with socket.create_connection(("127.0.0.1", port), 0.5): return True
        except OSError:
            time.sleep(0.25)
    return False

VER = sys.argv[1] if len(sys.argv) > 1 else "v1"

def render(k, port):
    tag = f"setup-{k:02d}"
    work = WORK / tag
    if work.exists(): shutil.rmtree(work)
    work.mkdir(parents=True)
    src = HERE / "cards" / tag
    for f in ("page.card", "page.data.json"):
        shutil.copy(src / f, work / f)
    shutil.copytree(src / "kit", work / "kit")
    data = work / "page.data.json"
    data.write_text(data.read_text().replace("127.0.0.1:8170", f"127.0.0.1:{ART_PORT}"))
    request = {"card": str(work / "page.card"), "data": str(data), "kit_dir": str(work / "kit"),
               "format": "l0-kit", "width": 406, "height": 776, "nonce": f"gate-b-setup-{tag}-{VER}",
               "result": str(work / "native.json"), "layout": str(work / "layout.json"),
               "actions": str(work / "actions.json")}
    (work / "request.json").write_text(json.dumps(request))
    log = (work / "host.log").open("w")
    env = {**os.environ, "MAKEPAD_HIDE_WINDOWS": "1", "BEAUTY_REQUEST": str(work / "request.json")}
    proc = subprocess.Popen([str(BEAUTY), "--remote", str(port)], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    png = None
    try:
        if not wait_port(port):
            return {"scene": tag, "error": "no port"}
        time.sleep(6)
        snap = subprocess.check_output(
            ["curl", "-s", "--max-time", "10", f"127.0.0.1:{port}/snap?all=1"]).decode()
        (EVIDENCE / f"{tag}-snap-{VER}.json").write_text(snap)
        grab = json.loads(subprocess.check_output(
            ["curl", "-s", "--max-time", "25", f"127.0.0.1:{port}/g"]).decode() or "{}")
        png = grab.get("png")
        if png and Path(png).is_file():
            shutil.copy(png, EVIDENCE / f"{tag}-native-{VER}.png")
        subprocess.run(["curl", "-s", "--max-time", "5", f"127.0.0.1:{port}/quit"],
                       capture_output=True)
        time.sleep(1)
    finally:
        if proc.poll() is None: proc.kill()
        log.close()
    text = (work / "host.log").read_text(errors="ignore")
    return {"scene": tag, "port": port, "png": str(png),
            "font_warnings": text.count("not available in this build"),
            "host_errors": text.count("error")}

def free_port(cands):
    for c in cands:
        t = socket.socket()
        try:
            t.bind(("127.0.0.1", c)); return c
        except OSError:
            continue
        finally:
            t.close()
    raise SystemExit("no free port in " + repr(cands))

if __name__ == "__main__":
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    handler = partial(Quiet, directory=str(PUBLISHED))
    httpd = ThreadingHTTPServer(("127.0.0.1", ART_PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    time.sleep(1)
    # Port hygiene (v6 lesson: another lane's beauty-host held 8391 and my /snap
    # hit THEIR scene). Bind-test every render port immediately before use.
    RENDER_PORTS = []
    cands = list(range(8390, 8400))
    for _ in range(6):
        rp = free_port(cands)
        RENDER_PORTS.append(rp)
        cands.remove(rp)
    print("render ports:", RENDER_PORTS, file=sys.stderr)
    try:
        for k, rp in zip(range(7, 13), RENDER_PORTS):
            out = render(k, rp)
            png = out.get("png")
            # A foreign host reports ITS grab dir; ours is always under ROOT/tmp.
            if png and not Path(png).resolve().is_relative_to(ROOT):
                out["error"] = "foreign host grabbed the port: " + png
            print(json.dumps(out))
    finally:
        httpd.shutdown()
