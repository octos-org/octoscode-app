#!/usr/bin/env python3
"""A15 — history on open, titles, the strip's model and the new-chat defaults,
walked by CLICKS in the real hidden app against RECORDED traffic.

The walk owns both processes and always stops them: `replay_serve <rport>
--scenario history` (r43a's recorded canonical hydrate — six turns, their
tool envelopes — answers every `session/hydrate` of the recorded Session; a
New chat has none; the catalog names the Session by its first prompt) and
the hidden app, with every app store in a per-run dir (brief §8).

  1. startup (no click): the Session's history is on screen — prompts,
     named tool cards from the replayed envelopes, answers — titled by the
     server's catalog in the header and the sidebar; the strip's model is
     the status read's; ONE session/hydrate went out after the open;
  2. CLICK New chat: an empty Session, hydrated too (empty); the defaults
     line says what a New chat gets (nothing stored: the server's defaults);
  3. CLICK the history row, New chat, the history row again: each open
     hydrates once, and the history shows once (the same rows, no copies);
  4. restart (stop + relaunch, same state): the history at startup;
  5. CLICK Settings > Permissions: the readback is the server's selection
     (Write · Network allowed matches no preset: no radio on).

usage: python3 tools/walk/a15_history_walk.py <host-bin> <desktop|phone> [port] [replay-port] [out-dir]
Exit status 0 when every check passes. Captures: <out-dir>/<mode>-NN-<name>.png
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARY = sys.argv[1] if len(sys.argv) > 1 else ""
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8429
RPORT = int(sys.argv[4]) if len(sys.argv) > 4 else 8441
OUT = os.path.abspath(sys.argv[5]) if len(sys.argv) > 5 else os.path.join(ROOT, "docs/ux/a15")
PHONE = MODE == "phone"
BASE = f"http://127.0.0.1:{PORT}"
STATE = tempfile.mkdtemp(prefix="a15walk.")
TRACE = os.path.join(STATE, "trace.jsonl")
CWD = "/home/user/octos"
HOME_SESSION = "dsflash:main"
TITLE = "Recording fixture. Reply with exactly: ok"
LAST_ANSWER = "Staged a peer, **Chip**"
RESULTS = []
SHOT_N = [0]


def get(path, timeout=20):
    for attempt in range(4):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except urllib.error.HTTPError:
            if path.startswith(("/click", "/t?", "/k?", "/m?")):
                time.sleep(0.3)
                return b"{}"
            if attempt == 3:
                raise
        except Exception:
            if attempt == 3:
                raise
        time.sleep(0.5)


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(s, wid):
    return [w for w in s if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def rect(wid, s=None, nth=0):
    hits = sorted(visible(s if s is not None else snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[nth]["r"] if len(hits) > nth else None


def text(wid, s=None):
    hits = sorted(visible(s if s is not None else snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[0].get("t", "") if hits else None


def texts(s=None):
    return [(w.get("i") or "", w.get("t") or "") for w in (s if s is not None else snap())
            if w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0 and w.get("t")]


def shown(wid, s=None):
    return rect(wid, s) is not None


def wait(pred, secs=10.0, period=0.3):
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
    x, y, w, h = r
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.5)


def click(wid, nth=0):
    r = rect(wid, nth=nth)
    if r:
        click_rect(r)
    return r is not None


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    print(f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else ""), flush=True)
    return ok


def shot(name):
    SHOT_N[0] += 1
    os.makedirs(OUT, exist_ok=True)
    p = os.path.join(OUT, f"{MODE}-{SHOT_N[0]:02d}-{name}.png")
    with open(p, "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", p, "--out", p], capture_output=True)
    print(f"  shot {os.path.relpath(p, ROOT)}", flush=True)


def trace():
    try:
        return [json.loads(l) for l in open(TRACE) if l.strip()]
    except FileNotFoundError:
        return []


def wire(method, session=None):
    """Outbound frames on the app's trace. `session/open` and the per-open
    `session/hydrate {include: [messages]}` are traced once each (flow.rs)."""
    out = [t.get("body") or {} for t in trace() if t.get("dir") == "out" and t.get("method") == method]
    return [b for b in out if session is None or b.get("session_id") == session]


def opens(session):
    return len(wire("session/open", session))


def message_hydrates(session):
    return len([b for b in wire("session/hydrate", session) if b.get("include") == ["messages"]])


def inside(inner, outer, slack=1.0):
    return (inner[0] >= outer[0] - slack and inner[1] >= outer[1] - slack
            and inner[0] + inner[2] <= outer[0] + outer[2] + slack
            and inner[1] + inner[3] <= outer[1] + outer[3] + slack)


# ------------------------------------------------------------------ processes
def env():
    e = dict(os.environ)
    for k, v in {
        "OCTOSCODE_DRAFTS_FILE": "drafts.json", "OCTOSCODE_CREDENTIALS_DIR": "cred",
        "OCTOSCODE_PREF_PATH": "prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": "notifications.json",
        "OCTOSCODE_RECENTS_DIR": "recents", "OCTOSCODE_SHOW_THINKING_FILE": "show-thinking.json",
        "OCTOSCODE_DOWNLOAD_DIR": "downloads", "OCTOSCODE_DISPLAY_PREFS_PATH": "display-v1.json",
        "OCTOSCODE_PANE_ADVANCED_FILE": "pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": "driver-id",
    }.items():
        e[k] = os.path.join(STATE, v)
    for d in ("cred", "downloads", "recents"):
        os.makedirs(os.path.join(STATE, d), exist_ok=True)
    e.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{RPORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "OCTOS_WORKSPACE_CWD": CWD,
        "OCTOSCODE_TRACE_FILE": TRACE,
        "OCTOSCODE_DESIGN_DIR": os.path.join(ROOT, "design"),
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": os.path.join(STATE, "hs"),
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if PHONE else ""),
    })
    if PHONE:
        e["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    return e


def start_app():
    subprocess.run(["bash", os.path.join(ROOT, "harness/headless.sh"), "start", BINARY, str(PORT)],
                   env=env(), cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return wait(lambda: shown("i0_composer_0"), 90, 1.0)


def stop_app():
    subprocess.run(["bash", os.path.join(ROOT, "harness/headless.sh"), "stop", str(PORT)], env=env(), cwd=ROOT,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def open_sidebar():
    """Phone: the sessions live in the drawer."""
    if PHONE and not shown("sb_new_chat_hit"):
        click("sidebar_toggle_hit")
        wait(lambda: shown("sb_new_chat_hit"), 4)


def history_on_screen(s=None):
    s = s if s is not None else snap()
    joined = " ".join(t for _, t in texts(s))
    return LAST_ANSWER in joined


def scroll_to_bottom():
    for _ in range(4):
        get(f"/m?k=scroll&x={600 if not PHONE else 180}&y=300&dy=600&wait=1")


# ------------------------------------------------------------------ the walk
def walk():
    # 1. Startup: the history, at once.
    check("app up against the recorded history", start_app())
    check("startup: the Session's history is on screen without a click", wait(history_on_screen, 15))
    s = snap()
    cards = [t for i, t in texts(s) if i.endswith("_toolcell_title")]
    check("startup: tool rows are the replayed cards, named", "peer_handoff" in cards and "peer_list" in cards, str(cards))
    check("startup: no tool output drawn as a 'System' notice", not any(t == "System" for _, t in texts(s)))
    title = text("hd_title", s)
    if not PHONE:
        check("title: the header names the Session from the server's catalog", title == TITLE, repr(title))
    check("strip: the model is the status read's", text("b3_strip_model", s) == "deepseek-v4-flash", repr(text("b3_strip_model", s)))
    for wid in ("i1_toolcell_title", "i23_assistantprose"):
        r = rect(wid, s)
        if r:
            # The conversation column: right of the desktop sidebar (x >= 335)
            # inside the 990 px window, or the phone's 360 px with >= 12 px gutters.
            lo, hi = (335, 1045 - 12) if not PHONE else (12, 360 - 12)
            check(f"layout: {wid} inside the conversation column", lo <= r[0] and r[0] + r[2] <= hi, f"{r} in [{lo}, {hi}]")
    tool = [w for w in s if (w.get("i") or "").endswith("_toolcell") and w.get("v", 1) != 0 and w["r"][3] > 0]
    if tool:
        check("layout: tool rows >= 28 px tall", all(w["r"][3] >= 28 for w in tool), str([w["r"][3] for w in tool]))
    check("wire: the startup open hydrated once", wait(lambda: message_hydrates(HOME_SESSION) == opens(HOME_SESSION) == 1, 5),
          f"opens {opens(HOME_SESSION)}, hydrates {message_hydrates(HOME_SESSION)}")
    shot("startup-history")
    baseline = sorted(t for i, t in texts(s) if "assistantprose" in i or "userbubble" in i or i.endswith("_toolcell_title"))

    # 2. New chat: empty, hydrated (empty), and the defaults line.
    open_sidebar()
    check("sidebar: the row is titled by the catalog", wait(lambda: text("sb_r_title") == TITLE, 6), repr(text("sb_r_title")))
    r = rect("sb_r_title")
    row = rect("sb_r_open") or r
    if r and row:
        check("layout: the row title stays inside its row", inside(r, row, 2.0), f"{r} in {row}")
    if not PHONE:
        shot("sidebar-titled")
    check("CLICK New chat", click("sb_new_chat_hit"))
    check("new chat: an empty Session", wait(lambda: shown("empty_title"), 10))
    new_id = next((b.get("session_id") for b in reversed(wire("session/open")) if b.get("session_id") != HOME_SESSION), None)
    check("wire: the new Session's open hydrated too", wait(lambda: new_id and message_hydrates(new_id) == 1, 6), str(new_id))
    line = text("hd_defaults_text")
    check("defaults line: what a New chat gets (nothing stored: the server's defaults)",
          line == ("New chat defaults · Server defaults · …" if PHONE else "New chat defaults · Server defaults · deepseek-v4-flash · Thinking: On"), repr(line))
    lr, strip = rect("hd_defaults_text"), rect("hd_defaults")
    if lr and strip:
        check("layout: the defaults line inside its strip", inside(lr, strip, 1.0), f"{lr} in {strip}")
        if not PHONE:
            check("layout: one line on desktop", lr[3] <= 20, str(lr))
    # (A13: the phone seat names the mode only, to keep the model whole.)
    seat = "Write" if PHONE else "Write · Network allowed"
    check("seat: the new Session's permissions are the server's", text("i0_composer_2_0") == seat, repr(text("i0_composer_2_0")))
    shot("new-chat-defaults")

    # 3. Back and forth: each open hydrates once, the history once.
    for rnd in (1, 2):
        open_sidebar()
        rows = [w for w in snap() if w.get("i") == "sb_r_title" and w.get("t") == TITLE and w["r"][3] > 0]
        check(f"round {rnd}: CLICK the history row", bool(rows) and (click_rect(rows[0]["r"]) or True))
        check(f"round {rnd}: the history shows again", wait(history_on_screen, 10))
        time.sleep(1.0)
        scroll_to_bottom()
        again = sorted(t for i, t in texts() if "assistantprose" in i or "userbubble" in i or i.endswith("_toolcell_title"))
        check(f"round {rnd}: the same rows, no copies", again == baseline, f"{len(again)} vs {len(baseline)}")
        open_sidebar()
        others = [w for w in snap() if w.get("i") == "sb_r_title" and w.get("t") == "New chat" and w["r"][3] > 0]
        check(f"round {rnd}: CLICK the New chat row", bool(others) and (click_rect(others[0]["r"]) or True))
        check(f"round {rnd}: the new Session is empty", wait(lambda: shown("empty_title"), 10))
    check("wire: one hydrate per open (history Session)", message_hydrates(HOME_SESSION) == opens(HOME_SESSION) == 3,
          f"opens {opens(HOME_SESSION)}, hydrates {message_hydrates(HOME_SESSION)}")
    check("wire: one hydrate per open (new Session)", new_id and message_hydrates(new_id) == opens(new_id) == 3,
          f"opens {opens(new_id)}, hydrates {message_hydrates(new_id)}")

    # 5. Settings > Permissions: the server's readback.
    opened = click("settings_open_hit")
    check("CLICK Settings", opened and wait(lambda: shown("settings_drawer"), 6))
    cell = rect("rl_hit" if PHONE else "nv_hit", nth=1)  # General, Permissions, …
    if cell:
        click_rect(cell)
    check("CLICK Permissions: the section shows", wait(lambda: shown("set_perm_readback"), 6))
    rb = text("set_perm_readback")
    check("readback: the server's selection and approval policy", rb == "Server: Write · Network allowed · asks on request", repr(rb))
    shot("permissions-readback")
    click("set_back" if PHONE else "settings_close")
    stop_app()

    # 4. Restart: the same state, the history at startup.
    check("restart: app up again", start_app())
    check("restart: the history is on screen without a click", wait(history_on_screen, 15))
    if not PHONE:
        check("restart: the header keeps the server's title", wait(lambda: text("hd_title") == TITLE, 6), repr(text("hd_title")))
    shot("restart-history")
    stop_app()


def main():
    replay = os.path.join(ROOT, "target/debug/examples/replay_serve")
    log = open(os.path.join(STATE, "replay.log"), "w")
    serve = subprocess.Popen([replay, str(RPORT), "--scenario", "history"], stdout=log, stderr=subprocess.STDOUT)
    time.sleep(1.5)
    code = 1
    try:
        if serve.poll() is not None:
            check("replay server up (port free?)", False)
        else:
            walk()
            failed = [r for r in RESULTS if not r[1]]
            print(f"== WALK a15 history {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed", flush=True)
            print(f"   state dir (trace, replay log): {STATE}", flush=True)
            code = 0 if not failed else 1
    finally:
        stop_app()
        serve.terminate()
        try:
            serve.wait(timeout=10)
        except subprocess.TimeoutExpired:
            serve.kill()
    return code


if __name__ == "__main__":
    sys.exit(main())
