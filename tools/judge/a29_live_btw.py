#!/usr/bin/env python3
"""A29 live proof: the /btw aside against a REAL octos (a PRIVATE serve this
script starts and stops — never the operator's), at most 3 model calls.

    python3 tools/judge/a29_live_btw.py <host-bin> [app-port 8606] [serve-port 50311]

1. A private `octos serve --solo` on 127.0.0.1:<serve-port> over a COPY of
   the live gate data dir (oa.noindex/live-gate/data, the dsflash profile), its token in a
   mode-600 file and only in the serve's environment (OCTOS_AUTH_TOKEN) and
   the app's (OCTOS_BEARER) — never printed, logged or saved. The served
   workspace is a fresh /tmp folder (deleted after), so no home path is
   rendered into a capture.
2. The hidden app (isolated state, OCTOSCODE_TRACE_FILE) opens Session X;
   `/btw <question>` is typed in X's composer + Return (the one model call).
   X and Y are both untitled ("New chat"), so they are told apart by the
   sidebar's selection and the trace's session ids, never by title.
3. While X's aside answers, a CLICK on the sidebar's New chat opens Session
   Y: Y shows no aside; the answer arrives while Y is on screen.
4. A CLICK on X's sidebar row: X's panel shows the answer.
5. The trace's session/btw frame carries X's id; the reply echoes it.
Always: the app and the serve stop, the data copy and the token file are
deleted, every saved file is grepped for the token (whole, first 8, last 8).

Evidence: docs/ux/a29/live/ (PNGs, checks.txt, the session/btw frames from
the trace, the app's aside log lines). No /snap JSON is saved (the instrument
reports masked fields' raw values).
"""
import json
import os
import re
import secrets
import shutil
import socket
import subprocess
import sys
import time
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../walk"))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = sys.argv[1]
APP_PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8606
SERVE_PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 50311
HOME = os.path.expanduser("~")
OCTOS = os.path.join(HOME, "home/oa.noindex/p0-build/tmp/octos-target/release/octos")
LIVE_DATA = os.path.join(HOME, "home/oa.noindex/live-gate/data")
WORK = os.path.join(ROOT, "tmp", "a29", f"live-{int(time.time())}")
# The RESOLVED temp dir (macOS /tmp is a symlink to /private/tmp): the server
# answers the open with the canonical root, and the app's exact-workspace
# check (row 204) refuses an open whose root differs from the one it asked.
WS = os.path.join(os.path.realpath("/tmp"), f"a29-ws-{secrets.token_hex(4)}")
OUT = os.path.join(ROOT, "docs", "ux", "a29", "live")
BASE = f"http://127.0.0.1:{APP_PORT}"
QUESTION = "In two sentences: why would a message queue drain fully before it resends after a reconnect?"
RESULTS = []
LOG = []


def note(line):
    print(line, flush=True)


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    line = f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else "")
    note(line)
    return ok


def get(path, timeout=20):
    with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
        return r.read()


def snap():
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w):
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid, sn=None, text=None):
    for w in (sn if sn is not None else snap()):
        if w.get("i") == wid and shown(w) and (text is None or (w.get("t") or "") == text):
            return w
    return None


def text_of(wid):
    w = find(wid)
    return (w or {}).get("t") or ""


def click_w(w):
    x, y, ww, hh = w["r"]
    for attempt in (0, 1):
        try:
            get(f"/click?x={x + ww / 2:.1f}&y={y + hh / 2:.1f}&wait=1")
            break
        except Exception:  # noqa: BLE001 — a missed frame: one retry
            if attempt:
                raise
            time.sleep(1.0)
    time.sleep(0.4)


def responsive(secs=60):
    """The bridge answers a command only between the app's event-loop turns
    (a busy loop times it out with a 404 "app busy" and KEEPS the command
    queued): wait for three quick /snap replies in a row."""
    end, quick = time.time() + secs, 0
    while time.time() < end and quick < 3:
        t0 = time.time()
        try:
            snap()
            quick = quick + 1 if time.time() - t0 < 1.5 else 0
        except Exception:  # noqa: BLE001
            quick = 0
        time.sleep(0.3)
    return quick >= 3


def composer_value():
    w = find("i0_composer_0")
    return (w or {}).get("val") or ""


def wait(pred, secs, period=0.4):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:  # noqa: BLE001 — a missed frame: retry
            pass
        time.sleep(period)
    return False


def pull_log():
    try:
        d = json.loads(get("/log?since=0"))
        LOG[:] = d.get("l", [])
    except Exception:  # noqa: BLE001
        pass


def capture(name):
    png = os.path.join(OUT, f"{name}.png")
    with open(png, "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    sn = snap()
    mod = next((w["r"] for w in sn if w.get("ty") == "OctoscodeView" and shown(w)), None)
    win = next((w["r"] for w in sn if w.get("ty") == "Window" and shown(w)), None)
    try:
        from PIL import Image

        img = Image.open(png)
        if mod and win:
            k = img.width / win[2]
            x, y, w, h = mod[0], max(mod[1] - 32, 0), mod[2], mod[3] + 32
            img.crop((int(x * k), int(y * k), int((x + w) * k), int((y + h) * k))).save(png)
    except Exception:  # noqa: BLE001 — keep the full grab
        pass
    subprocess.run(["sips", "-Z", "1400", png], capture_output=True)
    note(f"SHOT {name}")


def scrub(text):
    text = text.replace(ROOT, "<repo>").replace(WORK, "<live>").replace(WS, "<ws>")
    users = "/" + "Users" + "/"
    text = re.sub(re.escape(users) + r"[^\s\"',]+", "<abs>", text)
    return re.sub(r"/(private/)?var/folders/[^\s\"',]+", "<tmp>", text)


class Serve:
    def __init__(self):
        self.token = secrets.token_hex(16)
        self.proc = None

    def start(self):
        os.makedirs(WORK)
        shutil.copytree(LIVE_DATA, os.path.join(WORK, "data"))
        os.makedirs(os.path.join(WORK, "inst"))
        ws = WS
        os.makedirs(ws)
        with open(os.path.join(ws, "README.md"), "w") as f:
            f.write("A29 /btw live proof workspace.\n")
        tok = os.path.join(WORK, "token")
        with open(tok, "w") as f:
            f.write(self.token)
        os.chmod(tok, 0o600)
        env = dict(os.environ, OCTOS_AUTH_TOKEN=self.token)
        self.log = open(os.path.join(WORK, "serve.log"), "w")
        self.proc = subprocess.Popen(
            [OCTOS, "serve", "--port", str(SERVE_PORT), "--host", "127.0.0.1", "--solo",
             "--data-dir", os.path.join(WORK, "data"), "--instance-data-dir", os.path.join(WORK, "inst"), "--cwd", ws],
            stdout=self.log, stderr=subprocess.STDOUT, env=env, cwd=ws)
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

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(20)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        shutil.rmtree(WORK, ignore_errors=True)
        shutil.rmtree(WS, ignore_errors=True)
        note("serve stopped; the data copy, the workspace and the token file deleted")


def app_env(serve):
    st = os.path.join(WORK, "app-state")
    for d in ("cred", "downloads", "recents"):
        os.makedirs(os.path.join(st, d), exist_ok=True)
    e = dict(os.environ)
    e.update({
        "OCTOSCODE_DRAFTS_FILE": f"{st}/drafts.json", "OCTOSCODE_CREDENTIALS_DIR": f"{st}/cred",
        "OCTOSCODE_PREF_PATH": f"{st}/prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": f"{st}/notifications.json",
        "OCTOSCODE_SHOW_THINKING_FILE": f"{st}/show-thinking.json", "OCTOSCODE_DOWNLOAD_DIR": f"{st}/downloads",
        "OCTOSCODE_RECENTS_DIR": f"{st}/recents", "OCTOSCODE_DISPLAY_PREFS_PATH": f"{st}/display-v1.json",
        "OCTOSCODE_PANE_ADVANCED_FILE": f"{st}/pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": f"{st}/driver-id",
        "OCTOSCODE_CONNECTION_FILE": f"{st}/connection-v1.json",
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SERVE_PORT}", "OCTOS_BEARER": serve.token,
        "OCTOS_PROFILE_ID": "dsflash", "OCTOS_WORKSPACE_CWD": WS,
        "OCTOSCODE_DESIGN_DIR": os.path.join(ROOT, "design"), "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": os.path.join(WORK, "hs"), "HEADLESS_ARGS": "--module octoscode",
        "OCTOSCODE_TRACE_FILE": os.path.join(WORK, "trace.raw.jsonl"), "OCTOSCODE_THEME": "light",
    })
    e.pop("OCTOSENSE_WINDOW_SIZE", None)
    return e


def main():
    os.makedirs(OUT, exist_ok=True)
    serve = Serve()
    env = None
    try:
        if not check("a private octos serve is up on its own port", serve.start(), f"127.0.0.1:{SERVE_PORT}"):
            return 1
        env = app_env(serve)
        subprocess.run(["bash", os.path.join(ROOT, "harness", "headless.sh"), "start", BIN, str(APP_PORT)], env=env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if not check("the hidden app is up and connected (composer shown)", wait(lambda: find("i0_composer_0") is not None, 90, 1.0)):
            return 1
        check("the app's event loop is responsive", responsive())
        x_sel = find("sb_r_sel")
        check("X is open and selected in the sidebar", bool(x_sel), f"{x_sel and x_sel['r']}")
        # The one model call: the aside, asked in X. The text is typed once
        # and READ BACK from the composer before Return (a timed-out bridge
        # command stays queued, so it is never re-sent blindly).
        click_w(find("i0_composer_0"))
        text = f"/btw {QUESTION}"
        try:
            get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
        except Exception:  # noqa: BLE001 — queued: the read-back decides
            pass
        typed = wait(lambda: composer_value() == text, 15)
        if not check("the composer holds exactly the typed /btw command", typed, repr(composer_value()[:60])):
            return 1
        responsive(20)
        try:
            get("/k?c=Return&wait=1")
        except Exception:  # noqa: BLE001 — queued: Answering… decides
            pass
        check("X: the panel shows Answering… above X's composer",
              wait(lambda: text_of("btw_aside_status_label") == "Answering…", 10), text_of("btw_aside_status_label"))
        capture("1-x-answering")
        # Switch to a new Session Y before the answer arrives.
        nc = find("sb_new_chat_hit")
        check("CLICK New chat while X's aside answers", bool(nc) and not find("btw_aside_answer"))
        if nc:
            click_w(nc)
        def y_selected():
            sel = find("sb_r_sel")
            rows = [w for w in snap() if w.get("i") == "sb_r_title" and shown(w)]
            return bool(sel) and len(rows) >= 2 and x_sel and abs(sel["r"][1] - x_sel["r"][1]) > 4

        y_ok = wait(lambda: y_selected() and not find("btw_aside"), 15)
        check("Y (a new Session) is selected and shows no aside", y_ok, f"selected={find('sb_r_sel') and find('sb_r_sel')['r']}")
        # The answer arrives while Y is on screen.
        pull_log()
        landed = wait(lambda: (pull_log() or True) and any("[octoscode] aside" in l and "Answered" in l for l in LOG), 60, 1.0)
        line = next((l for l in LOG if "[octoscode] aside" in l and "-> " in l), "")
        check("the answer arrived while Y was on screen (settled into X)", landed, scrub(line.strip())[:200])
        time.sleep(1.0)
        check("Y still shows no aside after the answer", not find("btw_aside"))
        sn = snap()
        chip = find("sb_r_aside", sn)
        chips = [w for w in sn if w.get("i") == "sb_r_aside" and shown(w)]
        on_band = lambda w, c: abs((c["r"][1] + c["r"][3] / 2) - (w["r"][1] + w["r"][3] / 2)) < 14  # noqa: E731
        sel = find("sb_r_sel", sn)
        rows = [w for w in sn if w.get("i") == "sb_r_title" and shown(w)]
        x_rows = [w for w in rows if chips and on_band(w, chips[0])]
        check("exactly one sidebar row carries the aside marker, and it is not the selected (Y) row",
              len(chips) == 1 and bool(x_rows) and bool(sel) and not on_band(sel, chips[0]),
              f"chips={[c['r'] for c in chips]} selected={sel and sel['r']}")
        capture("2-y-marker-on-x")
        # Back to X.
        if x_rows:
            click_w(x_rows[0])
        check("back on the marked row: the whole answer", wait(lambda: bool(text_of("btw_aside_answer")), 10),
              repr(text_of("btw_aside_answer"))[:160])
        capture("3-x-answered")
    finally:
        if env is not None:
            pull_log()
            subprocess.run(["bash", os.path.join(ROOT, "harness", "headless.sh"), "stop", str(APP_PORT)], env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        # The protocol trace: the session/btw frames (out + its result) and the opens.
        raw = os.path.join(WORK, "trace.raw.jsonl")
        frames = []
        if os.path.exists(raw):
            for line in open(raw):
                try:
                    v = json.loads(line)
                except Exception:  # noqa: BLE001
                    continue
                if v.get("method") in ("session/btw", "session/open"):
                    frames.append(v)
        btw_out = [f for f in frames if f.get("method") == "session/btw" and f.get("dir") == "out"]
        btw_in = [f for f in frames if f.get("method") == "session/btw" and f.get("dir") == "in"]
        opens = [f.get("body", {}).get("session_id") for f in frames if f.get("method") == "session/open" and f.get("dir") == "out"]
        if btw_out:
            sid = btw_out[0].get("body", {}).get("session_id")
            check("trace: session/btw carries X's id (the first Session opened)", bool(opens) and sid == opens[0],
                  f"session_id={sid} opens={opens}")
            check("trace: the reply echoes X's id", bool(btw_in) and btw_in[0].get("body", {}).get("session_id") == sid,
                  f"reply session_id={btw_in[0].get('body', {}).get('session_id') if btw_in else None}")
            check("trace: exactly one session/btw (one model call)", len(btw_out) == 1, f"{len(btw_out)}")
            open_frames = [f for f in frames if f.get("method") == "session/open" and f.get("dir") == "out"]
            y_frames = [f for f in open_frames if f.get("body", {}).get("session_id") != sid]
            if y_frames and btw_in:
                y_at, ans_at = y_frames[0].get("at_ms", 0), btw_in[0].get("at_ms", 0)
                check("trace: the answer arrived after Y's open went out (it landed while Y was on screen)",
                      ans_at > y_at, f"Y open at {y_at} ms, the answer at {ans_at} ms")
            y_i = next((i for i, o in enumerate(opens) if o != sid), None)
            check("trace: X opened, then Y while it answered, then X again",
                  bool(opens) and opens[0] == sid and y_i is not None and sid in opens[y_i + 1:], f"opens={opens}")
        with open(os.path.join(OUT, "trace-btw.jsonl"), "w") as f:
            for v in frames:
                f.write(scrub(json.dumps(v)) + "\n")
        with open(os.path.join(OUT, "app-aside.log"), "w") as f:
            f.write(scrub("\n".join(l for l in LOG if "[octoscode] aside" in l or "command /btw" in l
                                    or "thread.open" in l or "new chat" in l)) + "\n")
        serve.stop()
        leaked = []
        for name in os.listdir(OUT):
            p = os.path.join(OUT, name)
            if os.path.isfile(p) and name.endswith((".log", ".jsonl", ".json", ".txt", ".md")):
                body = open(p, errors="ignore").read()
                if any(t in body for t in (serve.token, serve.token[:8], serve.token[-8:])):
                    leaked.append(name)
                    os.remove(p)
        check("no saved file carries the token (whole / first 8 / last 8)", not leaked, f"removed {leaked}" if leaked else "0 hits")
        with open(os.path.join(OUT, "checks.txt"), "w") as f:
            for name, ok, detail in RESULTS:
                f.write(f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else "") + "\n")
    return 0 if all(ok for _, ok, _ in RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
