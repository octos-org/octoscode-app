#!/usr/bin/env python3
"""PR #1 — the resizable sidebar's CLICK walk (judge: drag the grip, measure what moves).

The desktop sidebar's right edge carries an 8 px grip (`sidebar_resize`); a drag
resizes the column within [280, min(520, window - 420)] (`sidebar::bounded_width`).
Each width is checked by the /snap rects: the sidebar took the drag, the
conversation column starts after it, the composer stays inside the column, and
the HEADER still fits — the view tabs never overlap an action pill and the
session title keeps a readable width (judged on the first build of the PR: at a
469 px pane "Copy as Markdown" covered "Trajectory" and the title was gone; the
pills now go icon-only, `chrome::header_actions_fit`). `phone` checks that the
grip is not offered on the compact shell (the drawer has no edge to drag).

  cargo build -p octoscode-module --example replay_serve
  python3 tools/walk/sidebar_resize_walk.py <host-bin> desktop|phone [port] [replay-port] [out-dir]

It launches the replay server and the hidden app and ALWAYS stops both.
Exit status 0 when every step passes.
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

ROOT = Path(__file__).resolve().parent.parent.parent
BINARY = sys.argv[1] if len(sys.argv) > 1 else None
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
PHONE = MODE == "phone"
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8690
RPORT = int(sys.argv[4]) if len(sys.argv) > 4 else 8691
OUT = Path(sys.argv[5]) if len(sys.argv) > 5 else ROOT / "docs" / "ux" / "sidebar-resize" / "walk"
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []
ACTIONS = ("copy_open_hit", "review_open_hit", "settings_open_hit")
TABS = ("hd_tab_chat_hit", "hd_tab_traj_hit")


def say(line):
    print(line, flush=True)
    with open(OUT / f"walk-{MODE}.log", "a") as f:
        f.write(line + "\n")


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    say(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
    return bool(ok)


def get(path, timeout=20):
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read().decode()
    except urllib.error.HTTPError:
        return ""
    except OSError:
        return ""


def snap():
    try:
        return json.loads(get("/snap?all=1") or "{}").get("s", [])
    except json.JSONDecodeError:
        return []


def rect(wid, ws=None):
    for w in ws if ws is not None else snap():
        if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0:
            return [round(v, 1) for v in w["r"]]
    return None


def wait(fn, secs, step=0.4):
    end = time.time() + secs
    while time.time() < end:
        v = fn()
        if v:
            return v
        time.sleep(step)
    return None


def overlap(a, b):
    return a[0] < b[0] + b[2] - 0.5 and b[0] < a[0] + a[2] - 0.5 and a[1] < b[1] + b[3] - 0.5 and b[1] < a[1] + a[3] - 0.5


def shot(name):
    for _ in range(12):
        try:
            with urllib.request.urlopen(BASE + "/g?raw=1", timeout=30) as r:
                (OUT / f"{MODE}-{name}.png").write_bytes(r.read())
                return
        except (urllib.error.HTTPError, OSError):
            time.sleep(0.5)


def drag(x, y, dx, steps):
    get(f"/m?k=down&x={x}&y={y}&wait=1")
    for i in range(1, steps + 1):
        get(f"/m?k=move&x={x + dx * i / steps:.1f}&y={y}&wait=1")
    get(f"/m?k=up&x={x + dx:.1f}&y={y}&wait=1")
    time.sleep(1.0)


def layout_checks(label, want_w=None):
    ws = snap()
    side, conv, comp = rect("threads_column", ws), rect("conversation_column", ws), rect("i0_composer_0", ws)
    if want_w is not None:
        check(f"{label}: the sidebar is {want_w:.0f} px", bool(side) and abs(side[2] - want_w) <= 6, f"{side}")
    check(f"{label}: the conversation column starts after the sidebar",
          bool(side and conv) and conv[0] >= side[0] + side[2] - 1, f"{side} | {conv}")
    check(f"{label}: the composer stays inside the conversation column",
          bool(comp and conv) and comp[0] >= conv[0] - 1 and comp[0] + comp[2] <= conv[0] + conv[2] + 1, f"{comp} in {conv}")
    tabs = [(t, rect(t, ws)) for t in TABS if rect(t, ws)]
    acts = [(a, rect(a, ws)) for a in ACTIONS if rect(a, ws)]
    hits = [f"{t}/{a}" for t, tr_ in tabs for a, ar in acts if overlap(tr_, ar)]
    check(f"{label}: no action pill covers a view tab", not hits, ", ".join(hits) or f"{len(tabs)} tabs, {len(acts)} pills")
    title = rect("hd_title", ws)
    check(f"{label}: the session title keeps >= 100 px", bool(title) and title[2] >= 100, f"{title}")
    right = conv[0] + conv[2] if conv else 0
    out = [a for a, r in acts if r[0] + r[2] > right + 1]
    check(f"{label}: every action pill sits inside the conversation column", not out, ", ".join(out))
    return side


def main():
    if not BINARY:
        print(__doc__)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f"walk-{MODE}.log").write_text("")
    work = ROOT / "tmp" / "walk" / f"sidebar-resize-{MODE}"
    work.mkdir(parents=True, exist_ok=True)
    serve = subprocess.Popen([str(ROOT / "target" / "debug" / "examples" / "replay_serve"), str(RPORT),
                              "--scenario", "surfaces"],
                             stdout=open(OUT / f"replay-{MODE}.log", "w"), stderr=subprocess.STDOUT)
    time.sleep(1.5)
    env = os.environ.copy()
    env.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{RPORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"),
        # Isolated app state (brief §8): never the operator's ~/.octoscode.
        "OCTOSCODE_DRAFTS_FILE": str(work / "drafts.json"),
        "OCTOSCODE_CREDENTIALS_DIR": str(work / "cred"),
        "OCTOSCODE_PREF_PATH": str(work / "prefs.json"),
        "OCTOSCODE_NOTIFICATIONS_FILE": str(work / "notifications.json"),
        "OCTOSCODE_DISPLAY_PREFS_PATH": str(work / "display-v1.json"),
        "OCTOSCODE_PANE_ADVANCED_FILE": str(work / "pane-advanced.json"),
        "OCTOSCODE_DRIVER_ID_PATH": str(work / "driver-id"),
        "OCTOSCODE_CONNECTION_FILE": str(work / "connection-v1.json"),
        "OCTOSCODE_RECENTS_DIR": str(work),
        "HEADLESS_STATE": str(work / "state"),
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if PHONE else ""),
    })
    if PHONE:
        env["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    code = 1
    try:
        subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "start", BINARY, str(PORT)],
                       env=env, capture_output=True, text=True, timeout=240)
        say(f"== sidebar resize walk ({MODE}) — app :{PORT}, replay :{RPORT}")
        up = check("module up: the composer is shown", wait(lambda: rect("i0_composer_0"), 60))
        if up and PHONE:
            time.sleep(1.5)
            check("phone: no resize grip on the compact shell", rect("sidebar_resize") is None, f"{rect('sidebar_resize')}")
        elif up:
            wait(lambda: rect("hd_tab_chat_hit"), 30)
            time.sleep(1.5)
            ws = snap()
            side, grip = rect("threads_column", ws), rect("sidebar_resize", ws)
            check("the grip sits on the sidebar's right edge",
                  bool(side and grip) and abs(grip[0] + grip[2] / 2 - (side[0] + side[2])) <= 6, f"{grip} vs {side}")
            layout_checks("280 px (default)", 280)
            shot("01-default")
            if grip:
                y = grip[1] + grip[3] / 2
                drag(grip[0] + grip[2] / 2, y, 140, 7)
                layout_checks("+140 px drag", 420)
                shot("02-wider")
                grip = rect("sidebar_resize") or grip
                drag(grip[0] + grip[2] / 2, y, 600, 10)
                side = layout_checks("+600 px drag (past the bound)")
                conv = rect("conversation_column")
                check("past the bound: the sidebar stops at 520 px or leaves the conversation >= 420 px",
                      bool(side and conv) and side[2] <= 521 and conv[2] >= 419, f"sidebar {side}, conversation {conv}")
                shot("03-bound")
                grip = rect("sidebar_resize") or grip
                drag(grip[0] + grip[2] / 2, y, -900, 10)
                layout_checks("a drag back past the minimum", 280)
        failed = [r for r in RESULTS if not r[1]]
        say(f"== {len(RESULTS) - len(failed)}/{len(RESULTS)} passed ({MODE})")
        code = 0 if not failed else 1
    finally:
        subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "stop", str(PORT)], capture_output=True, timeout=60)
        serve.terminate()
    return code


if __name__ == "__main__":
    sys.exit(main())
