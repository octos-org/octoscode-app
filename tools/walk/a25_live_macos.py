#!/usr/bin/env python3
"""A25 — the REAL macOS round trip of a desktop notification (rows 322/323).

    python3 tools/walk/a25_live_macos.py <host-bin> <outdir> [--request] [--background]

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
   Exit 3. Without --request it only reports the state. macOS records a prompt
   still open when the app quits as Denied (seen on macOS 26: unanswered for 10
   minutes, the next start read Denied), so the run keeps the app up until the
   operator answers: A25_ANSWER_WAIT seconds, default 3600 (exit 0 allowed, 4
   refused, 3 no answer). A Denied identity is never asked again:
   A25_APP_NAME / A25_BUNDLE_ID give the bundle a fresh one.
5. Granted: opts in, sends ONE short prompt; the turn finishes while the
   window is unfocused (a hidden window is never focused) and macOS shows the
   banner — captured (the banner's own frame only, nothing else on the
   screen). A New chat is opened, then the banner is CLICKED for real
   (a pointer click, hit-tested onto the element carrying our notice text;
   macOS lets an app come forward only for user input): the app
   is brought forward and the notice's Session reopens. Exit 0 on success.
   While the display is shared or mirrored macOS presents no banner (its
   default) but still lists the notice in Notification Center: the run then
   opens Notification Center and captures and clicks the same notice there.
   With --background the person leaves the prompted Session for a New chat
   one second after sending (a prompt that takes a few seconds), so its turn
   finishes in the BACKGROUND (e2e/attention.spec.ts:231-269): the banner
   must name THAT Session while another is on screen, and the click reopens
   it. Use the default first; --background is the stronger proof.
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
import urllib.error
import urllib.parse
import urllib.request
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT / "tools" / "walk"))
from a10_lib import scrub as scrub_paths  # noqa: E402

BIN = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else None
OUT = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "docs" / "ux" / "a25" / "live-macos"
REQUEST = "--request" in sys.argv
BACKGROUND = "--background" in sys.argv
WORK = ROOT / "tmp" / "a25-macos"
# A fresh identity gets a fresh macOS permission prompt (a Denied one never prompts again).
APP_NAME = os.environ.get("A25_APP_NAME", "OctosCode A25")
APP = WORK / f"{APP_NAME}.app"
BUNDLE_ID = os.environ.get("A25_BUNDLE_ID", "dev.octoscode.desktop.a25")
# --request: keep the app (and its prompt) up this long for the operator's answer; quitting earlier records Denied.
ANSWER_WAIT = float(os.environ.get("A25_ANSWER_WAIT", "3600"))
APP_PORT = int(os.environ.get("A25_PORT", "8512"))
SERVE_PORT = int(os.environ.get("A25_SERVE_PORT", "8514"))
HOME = pathlib.Path(os.path.expanduser("~"))
OCTOS = pathlib.Path(os.environ.get("A25_OCTOS_BIN", HOME / "home/oa.noindex/p0-build/tmp/octos-target/release/octos"))
LIVE_DATA = pathlib.Path(os.environ.get("A25_LIVE_DATA", HOME / "home/oa.noindex/live-gate/data"))
NOTICE_TEXT = "Return to OctosCode to review it."
PROMPT_A = "Reply with exactly the two words: notice test"
# --background: an answer that takes a few seconds, so the person can leave the Session first.
PROMPT_BG = "Write five short numbered sentences about the colour blue."

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
        "CFBundleName": APP_NAME,
        "CFBundleDisplayName": APP_NAME,
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
    note(f"bundle: {APP_NAME}.app ({BUNDLE_ID}), ad-hoc signed")


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
    """A read retries a missed frame; an INPUT is never re-sent — the bridge
    may have applied it already (bridgeauth.input_was_queued: its 404 then
    means "applied, no frame acknowledgement"), and a second click lands on
    whatever moved under the pointer."""
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
    return b""


def snap() -> list[dict]:
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w: dict) -> bool:
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid: str):
    return next((w for w in snap() if w.get("i") == wid and shown(w)), None)


def click(wid: str) -> bool:
    """CLICK once at the widget's rect (waiting a little for it to be drawn)."""
    for _ in range(5):
        w = find(wid)
        if w:
            x, y, ww, hh = w["r"]
            get(f"/click?x={x + ww / 2}&y={y + hh / 2}&wait=1")
            time.sleep(0.4)
            return True
        time.sleep(0.6)
    note(f"CLICK {wid}: not visible")
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
    title_a = header()
    click("i0_composer_0")
    get("/t?" + urllib.parse.urlencode({"t": PROMPT_BG if BACKGROUND else PROMPT_A, "wait": 1}))
    get("/k?c=Return&wait=1")
    if BACKGROUND:
        # Leave the Session at once: its turn finishes in the BACKGROUND
        # (e2e/attention.spec.ts:231-269 natively).
        time.sleep(1.0)
        click("sb_new_chat_hit")
        note(f"background: left {title_a!r} for a New chat while its turn runs")
    posted = wait_log("attention: notice octoscode-attention:", 120, mark)
    # Look for the banner AT ONCE: macOS takes a temporary banner off the screen after about 5 s.
    frames = []
    look_until = time.time() + 10
    while posted and not frames and time.time() < look_until:
        frames = nc("find", NOTICE_TEXT) or []
    where = "a banner"
    if frames:
        captured = capture_region(frames[0], "banner")
    elif posted:
        # macOS presents no banner while the display is shared or mirrored (its default) or under a
        # Focus, but still delivers the notice to Notification Center, where the same click opens it.
        opened = (nc("center", "open") or {}).get("open")
        note(f"no banner within 10 s; Notification Center {'opened' if opened else 'did not open'}")
        where = "listed in Notification Center"
        look_until = time.time() + 6
        while not frames and time.time() < look_until:
            frames = nc("find", NOTICE_TEXT) or []
        if frames:
            time.sleep(0.6)  # the panel slides in: capture where the notice settled
            frames = nc("find", NOTICE_TEXT) or frames
            captured = capture_region(frames[0], "center-notice")
    check("granted: the finished turn posted the notice (window unfocused)", bool(posted),
          scrub_paths(posted or "")[-200:])
    check(f"granted: macOS shows the notice ({where})", bool(frames), json.dumps(frames))
    if frames:
        check("granted: the notice captured (its own frame only)", captured)
    # The notice names its Session by title (a new Session's header read "New chat" when it was sent).
    session_a = re.search(r"notice (octoscode-attention:\S+) — (.*?): A background response", posted or "")
    notice_title = session_a.group(2) if session_a else ""
    sent = PROMPT_BG if BACKGROUND else PROMPT_A
    if BACKGROUND:
        check("background: the notice names the Session that was left (another one is on screen)",
              notice_title[:24] == sent[:24] and header() != notice_title,
              f"on screen {header()!r}, the notice's Session {notice_title!r}")
    failed = wait_log("was not shown", 0.5, mark)
    check("granted: macOS accepted it (no add error)", failed is None, scrub_paths(failed or ""))
    if not BACKGROUND:
        # Another Session in front, so the click must route.
        click("sb_new_chat_hit")
        time.sleep(3)
    note(f"header before the click: {header()!r} (the notice names {notice_title!r})")
    mark = len(LOG)
    # A real pointer click (hit-tested onto our notice): macOS lets an app come forward only for user
    # input, so an AXPress would deliver the click but never bring the app forward.
    pressed = nc("click", NOTICE_TEXT) or {}
    how = "a pointer click on our notice"
    if not pressed.get("clicked"):
        note(f"pointer click refused: {json.dumps(pressed)}; AXPress instead")
        pressed = nc("press", NOTICE_TEXT) or {}
        how = "AXPress on our notice"
    check(f"granted: the notice was clicked ({where}: {how})",
          bool(pressed.get("clicked") or pressed.get("pressed")), json.dumps(pressed))
    clicked = wait_log("clicked", 8, mark)
    check("granted: the platform delivered the click (UNUserNotificationCenter delegate)", bool(clicked),
          scrub_paths(clicked or ""))
    routed = wait_log("notice click ->", 8, mark)
    shown = header()
    check("granted: the app opened the notice's Session",
          bool(routed) and bool(notice_title) and notice_title.startswith(shown.rstrip("…")) and len(shown) > 3,
          f"{scrub_paths(routed or '')[-120:]} header={shown!r}")
    # Activation can land a moment after the click (and the panel closing can hand the front back).
    states = []
    until = time.time() + 4
    while time.time() < until:
        st = nc("front", BUNDLE_ID) or {}
        if not states or st != states[-1]:
            states.append(st)
        if st.get("front") == BUNDLE_ID:
            break
        time.sleep(0.25)
    check("granted: the app was brought forward", bool(states) and states[-1].get("front") == BUNDLE_ID,
          json.dumps(states))
    if (nc("center", "state") or {}).get("open"):
        nc("center", "close")
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
                frames = nc("find", APP_NAME) or []
                note(f"the permission prompt on screen: {json.dumps(frames)}")
                if frames:
                    capture_region(frames[0], "permission-prompt")
                grab("enabling")
                if ANSWER_WAIT:
                    note(f"waiting up to {ANSWER_WAIT:.0f}s for the operator to answer the prompt")
                answered = wait_log("requested: true", max(2.0, ANSWER_WAIT))
                note(f"PERMISSION PROMPT SHOWN — the operator must allow \"{APP_NAME}\" (the prompt, or System "
                     f"Settings > Notifications > {APP_NAME}), then re-run this script."
                     if not answered else f"answered: {scrub_paths(answered)}")
                rc = 0 if answered and "status: Granted" in answered else 4 if answered else 3
        elif status == "Denied":
            note(f"denied: allow \"{APP_NAME}\" in System Settings > Notifications, then re-run")
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
                frames = nc("find", APP_NAME) or []
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
