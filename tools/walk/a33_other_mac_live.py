#!/usr/bin/env python3
"""A33 — "another Mac": the standalone OctosCode streams ONE live dsflash turn.

    python3 tools/walk/a33_other_mac_live.py <app-binary> <outdir> --label <name> \\
        [--app-home <dir>] [--hide <dir> ...] [--expect-packaged]

<app-binary> is the standalone app: `target/debug/octoscode` of a from-source
build, or `OctosCode.app/Contents/MacOS/octoscode` of an unzipped bundle.

1. Starts a PRIVATE `octos serve` (port A33_SERVE_PORT, default 8735) on a
   COPY of the live gate's data dir (dsflash profile; $A33_LIVE_DATA overrides)
   with a fresh instance dir and a fresh mode-600 random token that reaches the
   serve only through ITS environment (OCTOS_AUTH_TOKEN) and the app only
   through its own (OCTOS_BEARER): never printed, logged or saved.
2. With --hide, renames every named directory to `<dir>.a33-unreachable` for
   the whole run (restored afterwards), so a build tree the binary was compiled
   from cannot be read: the bundle must stand on its own.
3. Launches the app HIDDEN (harness/headless.sh, MAKEPAD_HIDE_WINDOWS=1) on
   A33_PORT (default 8733) with HOME=<--app-home> (the other user's home: the
   app's state and design cache land there), connected through its
   environment, and checks: the module mounted in the standalone host, the
   design root and faces under that HOME, no resource failed to load, a
   packaged build said so (--expect-packaged), the composer shows.
4. Sends ONE short prompt and checks that it streams and completes and that the
   answer renders.
5. Always: stops the app and the serve, deletes the data copy, restores the
   hidden directories, scrubs machine paths from the saved log and trace, and
   checks no saved file carries the token (whole, first 8, last 8).

Evidence under <outdir>: PNG captures, checks.txt, app.log, trace.jsonl. No
/snap JSON is saved (the instrument reports masked fields' raw values).
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import secrets
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "tools" / "walk"))
import bridgeauth  # noqa: E402  (D10c: the bridge token on every request)
from a10_lib import scrub as scrub_paths  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("bin")
ap.add_argument("out")
ap.add_argument("--label", required=True)
ap.add_argument("--app-home", default=None, help="HOME for the app (the other user's home)")
ap.add_argument("--hide", action="append", default=[], help="a directory to make unreachable during the run")
ap.add_argument("--expect-packaged", action="store_true")
ARGS = ap.parse_args()

BIN = pathlib.Path(ARGS.bin)
OUT = pathlib.Path(ARGS.out)
OUT.mkdir(parents=True, exist_ok=True)
APP_PORT = int(os.environ.get("A33_PORT", "8733"))
SERVE_PORT = int(os.environ.get("A33_SERVE_PORT", "8735"))
REAL_HOME = pathlib.Path(os.path.expanduser("~"))
OCTOS = pathlib.Path(os.environ.get("A33_OCTOS_BIN", REAL_HOME / "home/oa.noindex/p0-build/tmp/octos-target/release/octos"))
LIVE_DATA = pathlib.Path(os.environ.get("A33_LIVE_DATA", REAL_HOME / "home/oa.noindex/live-gate/data"))
WORK = ROOT / "tmp" / "a33-live"
PROMPT = "Reply with one short sentence: what is 2 + 3?"
BASE = f"http://127.0.0.1:{APP_PORT}"
RESULTS: list[tuple[str, bool, str]] = []
NOTES: list[str] = []


def note(line: str) -> None:
    NOTES.append(line)
    print(f"[a33] {line}", flush=True)


def check(name: str, ok, detail: str = "") -> bool:
    RESULTS.append((name, bool(ok), detail))
    line = f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else "")
    print(line, flush=True)
    with open(OUT / "checks.txt", "a") as f:
        f.write(scrub_paths(line) + "\n")
    return bool(ok)


# ----------------------------------------------------------- the serve
class Serve:
    def __init__(self) -> None:
        self.dir = WORK / f"serve-{ARGS.label}-{int(time.time())}"
        self.token = secrets.token_hex(24)
        self.proc: subprocess.Popen | None = None

    def start(self) -> bool:
        self.dir.mkdir(parents=True)
        shutil.copytree(LIVE_DATA, self.dir / "data")
        (self.dir / "inst").mkdir()
        # The conversation header shows the workspace's full path: a short
        # neutral one keeps the build machine's paths out of the captures.
        self.ws = pathlib.Path(tempfile.mkdtemp(prefix="a33-ws-", dir="/tmp"))
        (self.ws / "README.md").write_text("A33 other-Mac check workspace.\n")
        tok = self.dir / "token"
        tok.write_text(self.token)
        os.chmod(tok, 0o600)
        octos = WORK / "octos"
        if not octos.exists():
            shutil.copy2(OCTOS, octos)
        env = dict(os.environ, OCTOS_AUTH_TOKEN=self.token)
        log = open(self.dir / "serve.log", "w")
        self.proc = subprocess.Popen(
            [str(octos), "serve", "--port", str(SERVE_PORT), "--host", "127.0.0.1", "--solo",
             "--data-dir", str(self.dir / "data"), "--instance-data-dir", str(self.dir / "inst"), "--cwd", str(self.ws)],
            stdout=log, stderr=subprocess.STDOUT, env=env, cwd=str(self.ws))
        end = time.time() + 40
        while time.time() < end:
            if self.proc.poll() is not None:
                return False
            try:
                socket.create_connection(("127.0.0.1", SERVE_PORT), timeout=0.5).close()
                time.sleep(0.5)
                return self.proc.poll() is None
            except OSError:
                time.sleep(0.4)
        return False

    def stop(self) -> None:
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(20)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        shutil.rmtree(self.dir, ignore_errors=True)
        if getattr(self, "ws", None):
            shutil.rmtree(self.ws, ignore_errors=True)
        note("serve stopped, the data copy, the workspace and the token file deleted")


# ------------------------------------------------------------- the app
def get(path: str, timeout: float = 20) -> bytes:
    for attempt in range(3):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if bridgeauth.input_was_queued(path, e):
                return b""
            if path.startswith(bridgeauth.INPUT_ROUTES) or attempt == 2:
                raise
            time.sleep(1.0)
        except OSError:
            if path.startswith(bridgeauth.INPUT_ROUTES) or attempt == 2:
                raise
            time.sleep(1.0)
    return b""


def snap() -> list[dict]:
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w: dict) -> bool:
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid: str, sn=None):
    for w in (sn if sn is not None else snap()):
        if w.get("i") == wid and shown(w):
            return w
    return None


def texts(sn=None):
    return [(w.get("i") or "", w.get("t") or "") for w in (sn if sn is not None else snap()) if shown(w) and w.get("t")]


def click(wid: str) -> bool:
    w = find(wid)
    if w:
        x, y, ww, hh = w["r"]
        get(f"/click?x={x + ww / 2}&y={y + hh / 2}&wait=1")
        time.sleep(0.4)
    return bool(w)


def wait(pred, secs: float, period: float = 0.5) -> bool:
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:  # noqa: BLE001
            pass
        time.sleep(period)
    return False


N = [0]


def capture(name: str) -> None:
    N[0] += 1
    png = OUT / f"{ARGS.label}-{N[0]:02d}-{name}.png"
    png.write_bytes(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", str(png), "--out", str(png)], capture_output=True)


def log_lines() -> list[str]:
    try:
        return json.loads(get("/log?n=2000")).get("l", [])
    except Exception:  # noqa: BLE001
        return []


def running(sn=None) -> bool:
    sn = sn if sn is not None else snap()
    return find("composer_stop_icon", sn) is not None or find("composer_stop_busy", sn) is not None


def idle(sn=None) -> bool:
    sn = sn if sn is not None else snap()
    return find("composer_send_icon", sn) is not None and not running(sn)


def prose() -> str:
    return " ".join(t for i, t in texts() if "assistantprose" in i or i.endswith("_prose"))


def trace_text() -> str:
    """The app's protocol trace so far (OCTOSCODE_TRACE_FILE)."""
    try:
        return (OUT / "trace.raw.jsonl").read_text(errors="ignore")
    except OSError:
        return ""


def app_env(serve: Serve) -> dict:
    e = dict(os.environ)
    if ARGS.app_home:
        e["HOME"] = ARGS.app_home
    for k in ("OCTOSCODE_DESIGN_DIR", "MAKEPAD_WM_TEST_APP", "HEADLESS_ARGS"):
        e.pop(k, None)
    e.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SERVE_PORT}",
        "OCTOS_BEARER": serve.token,
        "OCTOS_PROFILE_ID": "dsflash",
        "OCTOS_WORKSPACE_CWD": str(serve.ws),
        "OCTOSCODE_TRACE_FILE": str(OUT / "trace.raw.jsonl"),
        "HEADLESS_STATE": str(OUT / "hs"),
    })
    return e


def start_app(serve: Serve) -> None:
    subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "start", str(BIN), str(APP_PORT)], cwd=str(ROOT),
                   env=app_env(serve), stdout=open(OUT / "app-start.log", "a"), stderr=subprocess.STDOUT)


def stop_app() -> None:
    subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "stop", str(APP_PORT)], cwd=str(ROOT),
                   env=dict(os.environ, HEADLESS_STATE=str(OUT / "hs")),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main() -> int:
    (OUT / "checks.txt").write_text("")
    hidden: list[tuple[pathlib.Path, pathlib.Path]] = []
    serve = Serve()
    log: list[str] = []
    try:
        for d in ARGS.hide:
            p = pathlib.Path(d)
            if p.exists():
                away = p.with_name(p.name + ".a33-unreachable")
                p.rename(away)
                hidden.append((p, away))
        if ARGS.hide:
            check("the build trees are unreachable for the whole run",
                  all(not p.exists() for p, _ in hidden) and len(hidden) == len(ARGS.hide),
                  f"{len(hidden)} director{'y' if len(hidden) == 1 else 'ies'} renamed away")
        if not check("private octos serve is up (dsflash, a copy of the live-gate data)", serve.start()):
            return 1
        start_app(serve)
        up = wait(lambda: find("i0_composer_0") is not None, 90, 1.0)
        log = log_lines()
        joined = "\n".join(log)
        check("the module mounted in the standalone host",
              "[octoscode-desktop] module octoscode (OctosCode) mounted" in joined)
        home = ARGS.app_home or str(REAL_HOME)
        root_line = next((l for l in log if "[octoscode] design root:" in l), "")
        check("the design root is under the app's HOME", f"{home}/.octoscode/design" in root_line,
              scrub_paths(root_line.split(" - ", 1)[-1]))
        face_line = next((l for l in log if "font faces resolve under" in l), "")
        check("the kit faces resolve under that design root", f"{home}/.octoscode/design/ux/" in face_line,
              scrub_paths(face_line.split(" - ", 1)[-1]))
        if ARGS.expect_packaged:
            check("a packaged build: the embedded design tree and faces only",
                  "design: packaged build" in joined)
        failed = [l for l in log if "Failed to load resource" in l or "Could not load resource" in l]
        check("no resource failed to load", not failed, scrub_paths(failed[0])[:200] if failed else "")
        if not check("connected: the composer shows", up):
            capture("not-connected")
            return 1
        time.sleep(3)
        capture("connected")
        # ONE live turn.
        click("i0_composer_0")
        get("/t?" + urllib.parse.urlencode({"t": PROMPT, "wait": 1}))
        time.sleep(0.3)
        get("/k?" + urllib.parse.urlencode({"c": "return", "wait": 1}))
        # A one-line answer can stream in well under a second: the Stop state
        # is then over before a poll sees it, so the trace's turn/started
        # counts as the start too (the first other-Mac run missed the Stop).
        stop_seen = wait(lambda: running() or '"method":"turn/started"' in trace_text(), 30, 0.2) and running()
        started = stop_seen or '"method":"turn/started"' in trace_text()
        check("the turn starts (Stop shows, or the trace has turn/started)", started,
              "Stop seen" if stop_seen else "the turn was already over; the trace has turn/started")
        if stop_seen:
            time.sleep(0.5)
            capture("streaming")
        done = wait(lambda: idle() and len(prose()) > 0, 150, 1.0)
        check("the turn completes (the composer is idle again)", done)
        answer = prose()
        check("the answer renders (it says 5)", "5" in answer, answer[:120])
        tr = trace_text()
        streamed = all(k in tr for k in ('"method":"turn/started"', '"kind":"text"', '"kind":"stream_end"'))
        check("the turn streamed (trace: turn/started, a text stream, stream_end)", streamed)
        capture("answered")
        log = log_lines()
        return 0 if all(ok for _, ok, _ in RESULTS) else 1
    finally:
        stop_app()
        serve.stop()
        for p, away in hidden:
            if away.exists() and not p.exists():
                away.rename(p)
        if hidden:
            note(f"restored {len(hidden)} hidden director{'y' if len(hidden) == 1 else 'ies'}")
        keep = [l for l in log if any(k in l for k in ("[octoscode", "notifications", "Failed to load", "makepad-remote] listening"))]
        (OUT / "app.log").write_text(scrub_paths("\n".join(keep)) + "\n")
        raw = OUT / "trace.raw.jsonl"
        if raw.exists():
            (OUT / "trace.jsonl").write_text(scrub_paths(raw.read_text()))
            raw.unlink()
        shutil.rmtree(OUT / "hs", ignore_errors=True)
        (OUT / "app-start.log").unlink(missing_ok=True)
        tok = serve.token
        leaked = [p.name for p in OUT.rglob("*") if p.is_file() and p.suffix in (".log", ".jsonl", ".json", ".txt", ".md")
                  and any(t in p.read_text(errors="ignore") for t in (tok, tok[:8], tok[-8:]))]
        for name in leaked:
            (OUT / name).unlink()
        check("no saved file carries the token", not leaked, f"removed {leaked}" if leaked else "")
        passed = sum(1 for _, ok, _ in RESULTS if ok)
        line = f"== {passed}/{len(RESULTS)} checks passed ({ARGS.label})"
        print(line)
        with open(OUT / "checks.txt", "a") as f:
            f.write(line + "\n")
        (OUT / "notes.txt").write_text(scrub_paths("\n".join(NOTES)) + "\n")


if __name__ == "__main__":
    sys.exit(main())
