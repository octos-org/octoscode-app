#!/usr/bin/env python3
"""A22 — the cross-session Stop, LIVE: a hidden OctosCode app against a PRIVATE
octos serve (a6ea8505, the dsflash profile, its own data / instance / ws dirs,
its own mode-600 token — never the operator's :50190).

  1. a long dsflash turn streams in Session X;
  2. CLICK New chat: Session Y is on screen while X keeps streaming;
  3. "press Stop" in Y every way the window offers: the Stop control (it must
     not be shown — Y has no live turn), Escape, and the `/stop` command;
  4. X streams to its end; the app's frame trace (OCTOSCODE_TRACE_FILE) holds
     NO turn/interrupt for X's turn;
  5. positive control: back in X, a second long turn, CLICK the Stop control:
     turn/interrupt {session_id: X, turn_id: that turn} and the turn ends
     interrupted.

    LIVE_DIR=<dir with a mode-600 `token` and `ws/`> \\
      python3 tools/walk/a22_live_stop.py <host-bin> <app-port> <serve-url> <outdir>

The token is read from $LIVE_DIR/token and reaches the app only through its
environment (OCTOS_BEARER); it is never printed, logged or saved. Snaps are
scrubbed (tools/walk/snapsafe.scrub) and machine paths rewritten before they
are written. Two model turns.
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)
from snapsafe import scrub  # noqa: E402

BIN, PORT, SERVE, OUT = sys.argv[1], int(sys.argv[2]), sys.argv[3], os.path.abspath(sys.argv[4])
LIVE = os.path.abspath(os.environ["LIVE_DIR"])
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BASE = f"http://127.0.0.1:{PORT}"
STATE = os.path.join(OUT, "state")
TRACE = os.path.join(STATE, "trace.jsonl")
os.makedirs(STATE, exist_ok=True)
RESULTS = []
N = [0]
LONG = "Write the whole numbers from 1 to 1200 as English words, one per line, and nothing else."
LONG2 = "Write the whole numbers from 1 to 1200 as Roman numerals, one per line, and nothing else."


def get(path, timeout=30):
    """A read is retried; an input (/click, /t, /k, /m) never is: a click
    re-sent after a slow frame lands on whatever moved under the pointer."""
    once = path.startswith(("/click", "/t?", "/k?", "/m?"))
    for attempt in range(3):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except Exception:
            if once:
                time.sleep(0.5)
                return b"{}"
            if attempt == 2:
                raise
            time.sleep(0.5)


def snap():
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w):
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid, s=None):
    s = s if s is not None else snap()
    hits = sorted([w for w in s if w.get("i") == wid and shown(w)], key=lambda w: (w["r"][1], w["r"][0]))
    return hits[0] if hits else None


def texts(s=None):
    s = s if s is not None else snap()
    return [(w.get("i") or "", w.get("t") or "") for w in s if shown(w) and w.get("t")]


def wait(pred, secs=10.0, period=0.4):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(period)
    return False


def click_rect(r):
    get(f"/click?x={r[0] + r[2] / 2}&y={r[1] + r[3] / 2}&wait=1")
    time.sleep(0.4)


def click(wid):
    w = find(wid)
    if w:
        click_rect(w["r"])
    return w is not None


def key(code):
    get(f"/k?c={code}&wait=1")
    time.sleep(0.3)


def type_text(t):
    for ch in t:
        get("/t?" + urllib.parse.urlencode({"t": ch, "wait": "1"}))
    time.sleep(0.2)


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    line = f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else "")
    print(line, flush=True)
    with open(os.path.join(OUT, "checks.txt"), "a") as f:
        f.write(line + "\n")
    return ok


def redact(text):
    """This machine's paths out of a saved file: the live dir, then any path
    under the home or the temp directory (whole path token)."""
    text = text.replace(LIVE, "<live>")
    tmp = tempfile.gettempdir()
    for root, label in ((os.path.expanduser("~"), "<home>"), (os.path.realpath(tmp), "<tmp>"), (tmp, "<tmp>")):
        text = re.sub(re.escape(root) + r"[^\"\s]*", label, text)
    return text


def capture(name):
    N[0] += 1
    base = os.path.join(OUT, f"{N[0]:02d}-{name}")
    s = snap()
    with open(base + ".png", "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    # The header's folder line is a machine path: painted over (the final-live rule).
    try:
        from PIL import Image, ImageDraw
        img = Image.open(base + ".png").convert("RGB")
        win = find("main_window", s)
        scale = img.width / float(win["r"][2]) if win else 1.0
        d = ImageDraw.Draw(img)
        for w in s:
            if w.get("i") == "hd_path" and shown(w):
                x, y, ww, hh = w["r"]
                d.rectangle([x * scale - 2, y * scale - 2, (x + ww) * scale + 2, (y + hh) * scale + 2], fill=(255, 255, 255))
        img.save(base + ".png")
    except Exception as e:  # never ship an unredacted capture
        os.remove(base + ".png")
        print("capture redaction failed:", e)
    subprocess.run(["sips", "-Z", "1400", base + ".png", "--out", base + ".png"], capture_output=True)
    with open(base + ".snap.json", "w") as f:
        f.write(redact(json.dumps(scrub(s), ensure_ascii=False)))


def trace():
    out = []
    try:
        for l in open(TRACE):
            try:
                out.append(json.loads(l))
            except ValueError:
                pass
    except FileNotFoundError:
        pass
    return out


def sent(method):
    return [t.get("body") or {} for t in trace() if t.get("dir") == "out" and t.get("method") == method]


def terminal_at(turn):
    """(outcome, wall_ms) of `turn`'s terminal on the wire, if it arrived."""
    for t in trace():
        b = t.get("body") or {}
        if t.get("dir") == "in" and t.get("method") == "projection/envelope" and b.get("turn_id") == turn:
            p = b.get("payload") or {}
            if p.get("type") == "turn_terminal":
                return (p.get("data") or {}).get("outcome"), t.get("wall_ms")
    return None


def terminal_of(turn):
    for t in trace():
        b = t.get("body") or {}
        if t.get("dir") == "in" and t.get("method") == "projection/envelope" and b.get("turn_id") == turn:
            p = b.get("payload") or {}
            if p.get("type") == "turn_terminal":
                return (p.get("data") or {}).get("outcome")
    return None


def env():
    with open(os.path.join(LIVE, "token")) as f:
        tok = f.read().strip()
    e = dict(os.environ)
    st = STATE
    for d in ("cred", "downloads", "recents"):
        os.makedirs(os.path.join(st, d), exist_ok=True)
    e.update({
        "OCTOSCODE_DRAFTS_FILE": f"{st}/drafts.json", "OCTOSCODE_CREDENTIALS_DIR": f"{st}/cred",
        "OCTOSCODE_PREF_PATH": f"{st}/prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": f"{st}/notifications.json",
        "OCTOSCODE_SHOW_THINKING_FILE": f"{st}/show-thinking.json", "OCTOSCODE_DOWNLOAD_DIR": f"{st}/downloads",
        "OCTOSCODE_RECENTS_DIR": f"{st}/recents", "OCTOSCODE_DISPLAY_PREFS_PATH": f"{st}/display-v1.json",
        "OCTOSCODE_PANE_ADVANCED_FILE": f"{st}/pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": f"{st}/driver-id",
        "OCTOSCODE_CONNECTION_FILE": f"{st}/connection-v1.json",
        "OCTOS_BASE_URL": SERVE, "OCTOS_BEARER": tok, "OCTOS_PROFILE_ID": "dsflash",
        "OCTOS_WORKSPACE_CWD": os.path.join(LIVE, "ws"),
        "OCTOSCODE_DESIGN_DIR": os.path.join(ROOT, "design"), "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": os.path.join(STATE, "hs"),
        "OCTOSCODE_TRACE_FILE": TRACE,
        "HEADLESS_ARGS": "--module octoscode",
    })
    return e


def start_app():
    subprocess.run(["bash", "harness/headless.sh", "start", BIN, str(PORT)], cwd=ROOT, env=env(),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return wait(lambda: find("i0_composer_0") is not None, 90, 1.0)


def stop_app():
    subprocess.run(["bash", "harness/headless.sh", "stop", str(PORT)], cwd=ROOT, env=env(),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def streaming(s=None):
    s = s if s is not None else snap()
    return find("composer_stop_icon", s) is not None


def send(prompt):
    click("i0_composer_0")
    type_text(prompt)
    key("Return")


def inside(a, b):
    return a[0] >= b[0] - 2 and a[1] >= b[1] - 2 and a[0] + a[2] <= b[0] + b[2] + 2 and a[1] + a[3] <= b[1] + b[3] + 2


def rows(s=None):
    """[(row rect, selected, dot)] for the drawn session rows; dot is the
    visible status dot inside the row (run / wait / done / fail / idle / None)."""
    s = s if s is not None else snap()
    sel = [w["r"] for w in s if w.get("i") == "sb_r_sel" and shown(w)]
    out = []
    for w in s:
        if w.get("i") != "sb_r_open" or not shown(w):
            continue
        dot = None
        for name in ("run", "wait", "done", "fail", "idle"):
            if any(x.get("i") == f"sb_st_{name}" and shown(x) and inside(x["r"], w["r"]) for x in s):
                dot = name
        out.append((w["r"], any(inside(x, w["r"]) for x in sel), dot))
    return out


def other_dots(s=None):
    """The status dots of the session rows that are not the selected one."""
    return [d for _, selected, d in rows(s) if not selected]


def click_unselected_row():
    """CLICK the one session row that is not the selected Session's."""
    other = [r for r, selected, _ in rows() if not selected]
    if len(other) != 1:
        return False
    click_rect(other[0])
    return True


def row(title_part, s=None):
    s = s if s is not None else snap()
    for w in s:
        if w.get("i") == "sb_r_title" and shown(w) and title_part in (w.get("t") or ""):
            return w["r"]
    return None


def walk():
    if not check("app up against the private live serve", start_app()):
        return
    wait(lambda: find("hd_title") is not None, 20)
    x = (sent("session/open") or [{}])[0].get("session_id")
    check("X: the startup Session is open", bool(x), str(x))
    check("X: a fresh Session (its empty state shows; nothing from an earlier run)",
          wait(lambda: find("empty_title") is not None, 10))
    # 1. A long turn streams in X.
    n0 = len(sent("turn/start"))
    send(LONG)
    ok = wait(lambda: len(sent("turn/start")) > n0, 15) and wait(streaming, 30)
    check("X: the long turn streams (Stop shows)", ok)
    tx = sent("turn/start")[n0].get("turn_id") if len(sent("turn/start")) > n0 else None
    time.sleep(3)
    capture("x-streaming")
    # 2. CLICK New chat: Y on screen while X keeps streaming.
    click("sb_new_chat_hit")
    check("Y: New chat is on screen", wait(lambda: find("empty_title") is not None, 15))
    y = next((b.get("session_id") for b in reversed(sent("session/open")) if b.get("session_id") != x), None)
    check("Y: a different Session", bool(y) and y != x, f"{x} -> {y}")
    s = snap()
    check("Y: no Stop control (Y has no live turn); Send shows",
          find("composer_stop_icon", s) is None and find("composer_send_icon", s) is not None)
    check("X keeps working in the background: its sidebar row shows the running dot",
          terminal_of(tx) is None and wait(lambda: other_dots() == ["run"], 5), str(other_dots()))
    capture("y-while-x-streams")
    # 3. "Press Stop" in Y — at once, while X still streams: Escape, then the
    #    /stop command (its text is cleared after, so nothing is left typed).
    n_int = len(sent("turn/interrupt"))
    click("i0_composer_0")
    key("Escape")
    send("/stop")
    stop_ms = int(time.time() * 1000)
    for _ in range(6):
        key("Backspace")
    time.sleep(2.0)
    check("X was still streaming when Stop was pressed in Y (no terminal yet)",
          terminal_at(tx) is None or terminal_at(tx)[1] > stop_ms, str(terminal_at(tx)))
    check("wire: nothing interrupted from Y (no turn/interrupt at all)", len(sent("turn/interrupt")) == n_int,
          json.dumps(sent("turn/interrupt")))
    capture("y-after-stop")
    # 4. X streams to its end.
    done = wait(lambda: terminal_of(tx) is not None, 240, 1.0)
    check("X's turn ended", done, str(terminal_of(tx)))
    check("X's turn COMPLETED (never interrupted)", terminal_of(tx) == "completed", str(terminal_of(tx)))
    check("wire: no turn/interrupt names X's turn", not [b for b in sent("turn/interrupt") if b.get("turn_id") == tx],
          json.dumps(sent("turn/interrupt")))
    check("X's row reads done (completed in background) while Y is on screen",
          wait(lambda: other_dots() == ["done"], 10), str(other_dots()))
    # Back to X: CLICK its sidebar row (the session row that is not the
    # selected one — both may still read "New chat" until X's catalog title
    # is re-listed), then the whole answer is there.
    n_open = len(sent("session/open"))
    check("CLICK X's row in the sidebar", click_unselected_row())
    check("wire: X re-opened", wait(lambda: any(b.get("session_id") == x for b in sent("session/open")[n_open:]), 10))
    wait(lambda: find("empty_title") is None, 15)
    time.sleep(2)
    later = [b.get("session_id") for b in sent("session/open")[n_open:]]
    check("wire: exactly one open, X's", later == [x], str(later))
    last = [t for _, t in texts() if "one thousand two hundred" in t.lower()]
    check("X: the answer streamed to its end (its last line, 1200, is on screen)", bool(last),
          (last[-1][-60:] if last else "").replace("\n", " | "))
    capture("x-finished")
    # 5. Positive control: Stop in X stops X.
    n0 = len(sent("turn/start"))
    send(LONG2)
    ok = wait(lambda: len(sent("turn/start")) > n0, 15) and wait(streaming, 30)
    check("X: a second long turn streams", ok)
    tx2 = sent("turn/start")[n0].get("turn_id") if len(sent("turn/start")) > n0 else None
    time.sleep(3)
    stop = find("composer_stop_icon")
    if stop:
        click_rect(stop["r"])
    hit = wait(lambda: [b for b in sent("turn/interrupt") if b.get("turn_id") == tx2], 15)
    ints = [b for b in sent("turn/interrupt") if b.get("turn_id") == tx2]
    check("positive control: CLICK Stop in X sends turn/interrupt for X's turn under X",
          hit and ints[0].get("session_id") == x, json.dumps(ints))
    check("positive control: X's turn ends interrupted", wait(lambda: terminal_of(tx2) == "interrupted", 30),
          str(terminal_of(tx2)))
    time.sleep(1.5)
    capture("x-stopped")
    with open(os.path.join(OUT, "ids.json"), "w") as f:
        json.dump({"x": x, "y": y, "x_turn": tx, "x_turn_2": tx2}, f, indent=1)


def main():
    open(os.path.join(OUT, "checks.txt"), "w").close()
    try:
        walk()
    finally:
        stop_app()
    failed = [r for r in RESULTS if not r[1]]
    line = f"== LIVE a22 stop: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed"
    print(line, flush=True)
    with open(os.path.join(OUT, "checks.txt"), "a") as f:
        f.write(line + "\n")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
