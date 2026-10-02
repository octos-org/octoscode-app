#!/usr/bin/env python3
"""A25 — the REAL macOS round trip of a desktop notification (rows 322/323).

    python3 tools/walk/a25_live_macos.py <host-bin> <outdir> [--request]

1. Wraps the host binary in this agent's OWN app bundle, `tmp/a25-macos/
   OctosCode A25.app` (bundle id `dev.octoscode.desktop.a25`, ad-hoc signed;
   never the operator's OctosCode Dev.app) — UNUserNotificationCenter serves
   only a process that runs from an `.app`.
2. Starts a PRIVATE `octos serve` (port A25_SERVE_PORT, default 8514) on a
   COPY of the live gate's data dir (oa.noindex/live-gate/data, or
   $A25_LIVE_DATA) with a fresh instance dir and a
   fresh mode-600 random token (it reaches the app only through its
   environment; never printed, logged or saved), profile dsflash.
3. Launches the bundled binary HIDDEN (harness/headless.sh:
   MAKEPAD_HIDE_WINDOWS=1, isolated state) on A25_PORT (default 8512) and
   reads the permission the platform layer reports.
4. Not determined: with --request it clicks Settings > General > Desktop
   notifications, which asks macOS (requestAuthorization); the OS shows its
   permission prompt — the run captures the prompt (read through
   Accessibility, never pressed) and STOPS: the operator must allow
   "OctosCode A25" (in the prompt, or System Settings > Notifications).
   Exit 3. Without --request it only reports the state.
5. Granted: opts in, sends ONE short prompt; the turn finishes while the
   window is unfocused (a hidden window is never focused) and macOS shows the
   banner — captured (the banner's own frame only, nothing else on the
   screen). A New chat is opened, then the banner is CLICKED for real
   (Accessibility AXPress on the element carrying our notice text): the app
   is brought forward and the notice's Session reopens. Exit 0 on success.
6. Always: stops the app and the serve, deletes the data copy, scrubs machine
   paths from the saved log and trace, and checks no saved file carries the
   token.

At most 2 model turns. Evidence under <outdir>.
"""
from __future__ import annotations

import json
import os
import pathlib
import plistlib
import re
import secrets
import shutil
import socket
import subprocess
import sys
import time
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "tools" / "walk"))
from a10_lib import scrub as scrub_paths  # noqa: E402

BIN = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else None
OUT = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "docs" / "ux" / "a25" / "live-macos"
REQUEST = "--request" in sys.argv
WORK = ROOT / "tmp" / "a25-macos"
APP = WORK / "OctosCode A25.app"
BUNDLE_ID = "dev.octoscode.desktop.a25"
APP_PORT = int(os.environ.get("A25_PORT", "8512"))
SERVE_PORT = int(os.environ.get("A25_SERVE_PORT", "8514"))
HOME = pathlib.Path(os.path.expanduser("~"))
OCTOS = pathlib.Path(os.environ.get("A25_OCTOS_BIN", HOME / "home/oa.noindex/p0-build/tmp/octos-target/release/octos"))
LIVE_DATA = pathlib.Path(os.environ.get("A25_LIVE_DATA", HOME / "home/oa.noindex/live-gate/data"))
NOTICE_TEXT = "Return to OctosCode to review it."
PROMPT_A = "Reply with exactly the two words: notice test"

RESULTS: list[tuple[str, bool, str]] = []
NOTES: list[str] = []


def note(line: str) -> None:
    print(line, flush=True)
    NOTES.append(line)


def check(name: str, ok: bool, detail: str = "") -> bool:
    RESULTS.append((name, bool(ok), detail))
    note(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
    return ok


# ------------------------------------------------------------- the bundle
def make_bundle(bin_path: pathlib.Path) -> None:
    shutil.rmtree(APP, ignore_errors=True)
    macos = APP / "Contents" / "MacOS"
    macos.mkdir(parents=True)
    shutil.copy2(bin_path, macos / "octosense")
    info = {
        "CFBundleName": "OctosCode A25",
        "CFBundleDisplayName": "OctosCode A25",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleVersion": "0.1.0",
        "CFBundleShortVersionString": "0.1.0",
        "CFBundlePackageType": "APPL",
        "CFBundleExecutable": "octosense",
        "LSMinimumSystemVersion": "12.0",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }
    with open(APP / "Contents" / "Info.plist", "wb") as f:
        plistlib.dump(info, f)
    subprocess.run(["codesign", "-s", "-", "--force", str(APP)], capture_output=True)
    note(f"bundle: OctosCode A25.app ({BUNDLE_ID}), ad-hoc signed")


# ----------------------------------------------------------- the serve
class Serve:
    def __init__(self) -> None:
        self.dir = WORK / f"serve-{int(time.time())}"
        self.token = secrets.token_hex(16)
        self.proc: subprocess.Popen | None = None

    def start(self) -> bool:
        self.dir.mkdir(parents=True)
        shutil.copytree(LIVE_DATA, self.dir / "data")
        (self.dir / "inst").mkdir()
        ws = self.dir / "ws"
        ws.mkdir()
        (ws / "README.md").write_text("A25 notification check workspace.\n")
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
             "--data-dir", str(self.dir / "data"), "--instance-data-dir", str(self.dir / "inst"), "--cwd", str(ws)],
            stdout=log, stderr=subprocess.STDOUT, env=env, cwd=str(ws))
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
        note("serve stopped, the data copy deleted")


# ------------------------------------------------------------- the app
BASE = f"http://127.0.0.1:{APP_PORT}"


def get(path: str, timeout: float = 20) -> bytes:
    with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
        return r.read()


def snap() -> list[dict]:
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w: dict) -> bool:
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid: str):
    return next((w for w in snap() if w.get("i") == wid and shown(w)), None)


def click(wid: str) -> bool:
    """CLICK at the widget's rect; a transient instrument error (a 404 while
    the window re-lays out) is retried, never fatal."""
    for attempt in range(3):
        w = find(wid)
        if not w:
            time.sleep(0.6)
            continue
        x, y, ww, hh = w["r"]
        try:
            get(f"/click?x={x + ww / 2}&y={y + hh / 2}&wait=1")
            time.sleep(0.4)
            return True
        except Exception as e:  # noqa: BLE001
            note(f"CLICK {wid}: retry after {e}")
            time.sleep(0.8)
    note(f"CLICK {wid}: not visible or not clickable")
    return False


LOG: list[str] = []
LOG_SEQ = [0]


def pull() -> None:
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    LOG.extend(d.get("l", []))


def wait_log(needle: str, secs: float, since: int = 0) -> str | None:
    end = time.time() + secs
    while time.time() < end:
        pull()
        hit = next((l for l in LOG[since:] if needle in l), None)
        if hit:
            return hit
        time.sleep(0.5)
    return None


def header() -> str:
    w = find("hd_title")
    return (w or {}).get("t", "")


def start_app(serve: Serve) -> bool:
    state = OUT / "app-state"
    shutil.rmtree(state, ignore_errors=True)
    for sub in ("cred", "recents", "downloads"):
        (state / sub).mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.update({
        "HEADLESS_STATE": str(WORK / "hs"),
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"),
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_ARGS": "--module octoscode",
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SERVE_PORT}",
        "OCTOS_BEARER": serve.token,
        "OCTOS_PROFILE_ID": "dsflash",
        "OCTOS_WORKSPACE_CWD": str(serve.dir / "ws"),
        "OCTOSCODE_TRACE_FILE": str(OUT / "trace.raw.jsonl"),
        "OCTOSCODE_DRAFTS_FILE": str(state / "drafts.json"),
        "OCTOSCODE_CREDENTIALS_DIR": str(state / "cred"),
        "OCTOSCODE_PREF_PATH": str(state / "prefs.json"),
        "OCTOSCODE_NOTIFICATIONS_FILE": str(state / "notifications.json"),
        "OCTOSCODE_SHOW_THINKING_FILE": str(state / "show-thinking.json"),
        "OCTOSCODE_DOWNLOAD_DIR": str(state / "downloads"),
        "OCTOSCODE_RECENTS_DIR": str(state / "recents"),
        "OCTOSCODE_DISPLAY_PREFS_PATH": str(state / "display-v1.json"),
        "OCTOSCODE_PANE_ADVANCED_FILE": str(state / "pane-advanced.json"),
        "OCTOSCODE_DRIVER_ID_PATH": str(state / "driver-id"),
        "OCTOSCODE_CONNECTION_FILE": str(state / "connection-v1.json"),
        "OCTOSCODE_NOTIFY_FAKE": "",
    })
    (WORK / "hs").mkdir(parents=True, exist_ok=True)
    r = subprocess.run(["bash", str(ROOT / "harness" / "headless.sh"), "start",
                        str(APP / "Contents" / "MacOS" / "octosense"), str(APP_PORT)],
                       env=env, capture_output=True, text=True)
    if r.returncode != 0:
        note("app did not start: " + scrub_paths(r.stderr[-300:]))
        return False
    end = time.time() + 60
    while time.time() < end:
        try:
            if b'"sz"' in get("/s", 3):
                break
        except Exception:
            pass
        time.sleep(1)
    time.sleep(4)
    return True


def stop_app() -> None:
    subprocess.run(["bash", str(ROOT / "harness" / "headless.sh"), "stop", str(APP_PORT)],
                   env=dict(os.environ, HEADLESS_STATE=str(WORK / "hs")), capture_output=True)
    note("app stopped")


def grab(name: str) -> None:
    """The app's own capture, every shown label carrying a machine path (the
    workspace path in the header and in Settings) painted over first."""
    png = OUT / f"{name}.png"
    try:
        sn = snap()
        png.write_bytes(get("/g?raw=1", 30))
        from PIL import Image, ImageDraw

        img = Image.open(png).convert("RGB")
        win = next((w["r"] for w in sn if w.get("ty") == "Window" and shown(w)), None)
        k = img.width / win[2] if win else 1.0
        ox, oy = (win[0], win[1]) if win else (0, 0)
        draw = ImageDraw.Draw(img)
        machine = (str(HOME), "/private/", "/var/folders")
        for w in sn:
            t = str(w.get("t") or "")
            if shown(w) and w.get("ty") in ("Label", "TextInput") and any(m in t for m in machine):
                x, y, ww, hh = w["r"]
                draw.rectangle(((x - ox) * k - 2, (y - oy) * k - 2, (x + ww - ox) * k + 2, (y + hh - oy) * k + 2),
                               fill=(128, 128, 128))
        img.save(png)
        subprocess.run(["sips", "-Z", "1400", str(png)], capture_output=True)
    except Exception as e:  # noqa: BLE001 — never keep an unredacted capture
        png.unlink(missing_ok=True)
        note(f"grab {name} dropped (redaction failed: {e})")


# ------------------------------------------------- Notification Center
def nc_tool() -> pathlib.Path:
    exe = WORK / "a25_nc"
    src = ROOT / "tools" / "walk" / "a25_nc.swift"
    if not exe.exists() or exe.stat().st_mtime < src.stat().st_mtime:
        subprocess.run(["swiftc", "-O", str(src), "-o", str(exe)], check=True, capture_output=True)
    return exe


def nc(cmd: str, needle: str):
    r = subprocess.run([str(nc_tool()), cmd, needle], capture_output=True, text=True, timeout=30)
    try:
        return json.loads(r.stdout.strip() or "null")
    except json.JSONDecodeError:
        return None


def capture_region(frame: dict, name: str) -> bool:
    """screencapture of exactly `frame` (the banner / prompt), slightly inset so
    nothing behind it shows."""
    inset = 2
    x, y = int(frame["x"]) + inset, int(frame["y"]) + inset
    w, h = int(frame["w"]) - 2 * inset, int(frame["h"]) - 2 * inset
    png = OUT / f"{name}.png"
    r = subprocess.run(["screencapture", "-x", "-R", f"{x},{y},{w},{h}", str(png)], capture_output=True)
    return r.returncode == 0 and png.exists() and png.stat().st_size > 1000


def frontmost_bundle() -> str:
    try:
        front = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()
        info = subprocess.run(["lsappinfo", "info", "-only", "bundleid", front], capture_output=True, text=True).stdout
        m = re.search(r'"CFBundleIdentifier"="([^"]+)"', info) or re.search(r'bundleid"?="?([\w.\-]+)', info)
        return m.group(1) if m else info.strip()
    except Exception:  # noqa: BLE001
        return ""


# ---------------------------------------------------------------- the run
def authorization() -> str:
    line = wait_log("attention: os Authorization", 20)
    m = re.search(r"status: (\w+), requested: false", line or "")
    return m.group(1) if m else "none"


def run_granted() -> None:
    click("settings_open_hit")
    click("tg_notify")
    check("granted: the toggle opts in at once (no prompt: already allowed)",
          bool(wait_log("status: Granted, requested: true", 8)))
    click("settings_close")
    mark = len(LOG)
    composer = find("i0_composer_0")
    if not check("granted: the composer is ready", bool(composer)):
        return
    click("i0_composer_0")
    get("/t?" + urllib.parse.urlencode({"t": PROMPT_A, "wait": 1}))
    get("/k?c=Return&wait=1")
    posted = wait_log("attention: notice octoscode-attention:", 120, mark)
    check("granted: the finished turn posted the notice (window unfocused)", bool(posted),
          scrub_paths(posted or "")[-200:])
    session_a = re.search(r"notice (octoscode-attention:\S+)", posted or "")
    failed = wait_log("was not shown", 3, mark)
    check("granted: macOS accepted it (no add error)", failed is None, scrub_paths(failed or ""))
    time.sleep(1.5)
    frames = nc("find", NOTICE_TEXT) or []
    check("granted: the banner is on screen (Notification Center)", bool(frames), json.dumps(frames))
    if frames:
        check("granted: banner captured (its own frame only)", capture_region(frames[0], "banner"))
    title_a = header()
    # Another Session in front, so the click must route.
    click("sb_new_chat_hit")
    time.sleep(3)
    note(f"header before the click: {header()!r} (the notice names {title_a!r})")
    mark = len(LOG)
    pressed = nc("press", NOTICE_TEXT)
    check("granted: the banner was clicked (AXPress on our notice)", bool(pressed and pressed.get("pressed")), json.dumps(pressed))
    clicked = wait_log("clicked", 8, mark)
    check("granted: the platform delivered the click (UNUserNotificationCenter delegate)", bool(clicked),
          scrub_paths(clicked or ""))
    routed = wait_log("notice click ->", 8, mark)
    check("granted: the app opened the notice's Session", bool(routed) and (header() == title_a),
          f"{scrub_paths(routed or '')[-120:]} header={header()!r}")
    check("granted: the app was brought forward", frontmost_bundle() == BUNDLE_ID, frontmost_bundle())
    grab("after-click")
    if session_a:
        note(f"notice id: {session_a.group(1)}")


def main() -> int:
    if not BIN or not BIN.exists():
        print(__doc__)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    WORK.mkdir(parents=True, exist_ok=True)
    make_bundle(BIN)
    serve = Serve()
    rc = 1
    try:
        if not check("private octos serve is up", serve.start()):
            return 1
        if not check("the bundled app is up (hidden)", start_app(serve)):
            return 1
        pull()
        start = next((l for l in LOG if "attention: start" in l), "")
        check("the app runs with the platform notification API and is not focused",
              "platform api true, focused false" in start, scrub_paths(start)[-90:])
        status = authorization()
        note(f"macOS authorization for {BUNDLE_ID}: {status}")
        if status == "Unavailable":
            check("the bundle is served by UNUserNotificationCenter", False, "reported Unavailable")
        elif status == "NotDetermined":
            if not REQUEST:
                note("not determined: run with --request to let macOS ask (the operator must answer)")
                rc = 3
            else:
                click("settings_open_hit")
                click("tg_notify")
                time.sleep(1.0)
                state = (find("notify_state") or {}).get("t", "")
                check("request: the row reads Enabling… while macOS asks", state == "Enabling…", state)
                time.sleep(3.0)
                frames = nc("find", "OctosCode A25") or []
                note(f"the permission prompt on screen: {json.dumps(frames)}")
                if frames:
                    capture_region(frames[0], "permission-prompt")
                answered = wait_log("requested: true", 2)
                note("PERMISSION PROMPT SHOWN — the operator must allow \"OctosCode A25\" (the prompt, or System "
                     "Settings > Notifications > OctosCode A25), then re-run this script."
                     if not answered else f"answered: {scrub_paths(answered)}")
                grab("enabling")
                rc = 3
        elif status == "Denied":
            note("denied: allow \"OctosCode A25\" in System Settings > Notifications, then re-run")
            if REQUEST:
                # A decided permission is never asked again: the request
                # answers at once (the real completion handler) with no UI.
                mark = len(LOG)
                click("settings_open_hit")
                click("tg_notify")
                answered = wait_log("requested: true", 10, mark)
                check("denied: the request answers at once from the real center",
                      bool(answered) and "status: Denied" in answered, scrub_paths(answered or ""))
                time.sleep(1.0)
                frames = nc("find", "OctosCode A25") or []
                check("denied: macOS showed no prompt", not frames, json.dumps(frames))
                alert = (find("notify_alert") or {}).get("t", "")
                check("denied: the row reads the blocked alert, the toggle stays off",
                      alert.startswith("Notifications are blocked") and bool(find("tg_notify")), alert)
                grab("denied-blocked")
            rc = 4
        elif status == "Granted":
            run_granted()
            rc = 0 if all(ok for _, ok, _ in RESULTS) else 1
    finally:
        try:
            pull()
        except Exception:  # noqa: BLE001
            pass
        stop_app()
        serve.stop()
        (OUT / "app.log").write_text(scrub_paths("\n".join(l for l in LOG if "[octoscode]" in l or "notifications:" in l)) + "\n")
        raw = OUT / "trace.raw.jsonl"
        if raw.exists():
            (OUT / "trace.jsonl").write_text(scrub_paths(raw.read_text()))
            raw.unlink()
        shutil.rmtree(OUT / "app-state", ignore_errors=True)
        leaked = [str(p.name) for p in OUT.rglob("*") if p.is_file() and p.suffix in (".log", ".jsonl", ".json", ".txt")
                  and any(t in p.read_text(errors="ignore") for t in (serve.token, serve.token[:8], serve.token[-8:]))]
        if leaked:
            for name in leaked:
                (OUT / name).unlink()
        check("no saved file carries the token", not leaked, f"removed {leaked}" if leaked else "")
        (OUT / "result.txt").write_text("\n".join(NOTES) + "\n")
    return rc


if __name__ == "__main__":
    sys.exit(main())
