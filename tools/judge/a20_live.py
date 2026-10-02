#!/usr/bin/env python3
"""A20 live: parity rows 250 + 247 on a PRIVATE octos serve (never the operator's).

    LIVE_DIR=<dir holding a mode-600 `token`, the served `ws/` (with `scratch_dir/`) and `link-ws` -> ws> \\
      python3 tools/judge/a20_live.py <host-bin> <app-port> <serve-url> <outdir> [desktop|phone]

Row 250 (one model turn): Session X asks to run `rm -rf scratch_dir` in ask mode -> the approval card shows in X.
"New chat" opens Session Y: no card in Y, Y / S / N pressed in Y send nothing, the sidebar shows X waiting (not Y).
Back on X (a sidebar CLICK): X's card again; "Approve once" answers it with X's ids; scratch_dir is gone on disk.
Row 247 (no model turn): the app relaunches with a saved link naming Session Y in `link-ws` (a symlink the server
resolves to `ws`): "Open conversation" is refused on the panel, and the protocol trace shows the catalog read for
`link-ws` but NO session/open and NO session/hydrate for Y. "Dismiss link". Then the exact workspace opens Y.

The token is read from $LIVE_DIR/token and reaches the app only through its environment (OCTOS_BEARER); it is never
printed, logged or saved. No /snap JSON is written (the instrument reports masked fields' raw values); only PNG
captures, <outdir>/checks.txt and the app's protocol trace <outdir>/trace.jsonl.
"""
import json
import os
import subprocess
import sys
import time
import urllib.parse
import urllib.request
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../walk"))  # noqa: E402
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

BIN, PORT, SERVE, OUT = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
MODE = sys.argv[5] if len(sys.argv) > 5 else "desktop"
PHONE = MODE == "phone"
LIVE = os.environ["LIVE_DIR"]
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BASE = f"http://127.0.0.1:{PORT}"
STATE = os.path.join(OUT, "state")
TRACE = os.path.join(OUT, "trace.jsonl")
os.makedirs(OUT, exist_ok=True)
RESULTS = []
N = [0]
COMMAND = "rm -rf scratch_dir"
PROMPT = f"Use your bash tool to run exactly this command, nothing else, and do not ask me first: {COMMAND}"


def get(path, timeout=15):
    for attempt in (0, 1):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except Exception:
            if attempt:
                raise
            time.sleep(1.5)


def snap():
    d = json.loads(get("/snap?all=1"))
    return d.get("s", d) if isinstance(d, dict) else d


def shown(w):
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def find(wid, sn=None):
    for w in (sn if sn is not None else snap()):
        if w.get("i") == wid and shown(w):
            return w
    return None


def text(wid, sn=None):
    w = find(wid, sn)
    return (w.get("t") or "") if w else None


def click_w(w):
    x, y, ww, hh = w["r"]
    get(f"/click?x={x + ww / 2}&y={y + hh / 2}&wait=1")
    time.sleep(0.6)


def click(wid, nth=0):
    hits = sorted([w for w in snap() if w.get("i") == wid and shown(w)], key=lambda w: (w["r"][1], w["r"][0]))
    if len(hits) > nth:
        click_w(hits[nth])
        return True
    return False


def type_text(t):
    get("/t?" + urllib.parse.urlencode({"t": t, "wait": 1}))
    time.sleep(0.3)


def key(c):
    get("/k?" + urllib.parse.urlencode({"c": c, "wait": 1}))
    time.sleep(0.3)


def wait(pred, secs, period=0.5):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(period)
    return False


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    line = ("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else "")
    print(line, flush=True)
    with open(os.path.join(OUT, "checks.txt"), "a") as f:
        f.write(line + "\n")
    return bool(ok)


def capture(name):
    N[0] += 1
    png = os.path.join(OUT, f"{MODE}-{N[0]:02d}-{name}.png")
    with open(png, "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", png, "--out", png], capture_output=True)


def trace():
    out = []
    try:
        with open(TRACE) as f:
            for line in f:
                try:
                    out.append(json.loads(line))
                except ValueError:
                    pass
    except FileNotFoundError:
        pass
    return out


def wire(method, session=None):
    return [t.get("body") or {} for t in trace() if t.get("dir") == "out" and t.get("method") == method
            and (session is None or (t.get("body") or {}).get("session_id") == session)]


def env(extra=None):
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
        "HEADLESS_STATE": os.path.join(OUT, "hs"),
        "OCTOSCODE_TRACE_FILE": TRACE,
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if PHONE else ""),
    })
    if PHONE:
        e["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    if extra:
        e.update(extra)
    return e


def start_app(extra=None):
    subprocess.run(["bash", "harness/headless.sh", "start", BIN, str(PORT)], cwd=ROOT, env=env(extra),
                   stdout=open(os.path.join(OUT, "app-start.log"), "a"), stderr=subprocess.STDOUT)
    up = wait(lambda: find("i0_composer_0") is not None, 90, 1.0)
    if not up and PHONE:
        icon = next((w for w in snap() if (w.get("t") or "").strip() == "OctosCode" and shown(w)), None)
        if icon:
            click_w(icon)
        up = wait(lambda: find("i0_composer_0") is not None, 60, 1.0)
    return up


def stop_app():
    subprocess.run(["bash", "harness/headless.sh", "stop", str(PORT)], cwd=ROOT, env=env(),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def sidebar_open():
    if PHONE and find("sb_new_chat_hit") is None:
        click("sidebar_toggle_hit")
        wait(lambda: find("sb_new_chat_hit") is not None, 4)


def sidebar_close():
    if PHONE and find("sb_new_chat_hit") is not None:
        click("sidebar_toggle_hit")
        time.sleep(0.8)


def row_of(session_title_prefix, sn=None):
    sn = sn if sn is not None else snap()
    return next((w for w in sn if w.get("i") == "sb_r_title" and shown(w)
                 and (w.get("t") or "").startswith(session_title_prefix)), None)


def row_waits(row, sn):
    cy = row["r"][1] + row["r"][3] / 2
    return any(w.get("i") == "sb_st_wait" and shown(w) and abs(w["r"][1] + w["r"][3] / 2 - cy) < 14 for w in sn)


def send(prompt):
    if not click("i0_composer_0"):
        return False
    type_text(prompt)
    key("return")
    return True


def phase_250():
    if not check("app up against the private serve", start_app()):
        return None
    check("connected: the Chat tab shows", wait(lambda: find("hd_tab_chat_hit") is not None, 30))
    time.sleep(3)
    x = next((b.get("session_id") for b in wire("session/open")), None)
    check("Session X is the startup Session", bool(x), str(x))
    # Ask mode (Settings > Permissions > Ask for approval), as A15 proved it.
    check("CLICK Settings", click("settings_open_hit") and wait(lambda: find("settings_drawer") is not None, 6))
    click("rl_hit" if PHONE else "nv_hit", 1)
    check("CLICK Permissions", wait(lambda: find("set_perm_readback") is not None, 6))
    check("CLICK Ask for approval", click("perm_ask"))
    check("readback: the server asks on request", wait(lambda: "asks on request" in (text("set_perm_readback") or ""), 10),
          repr(text("set_perm_readback")))
    click("set_back" if PHONE else "settings_close")
    time.sleep(1.0)
    scratch = os.path.join(LIVE, "ws", "scratch_dir")
    check("scratch_dir exists before", os.path.isdir(scratch))
    # Turn 1 (the only model turn of this run).
    send(PROMPT)
    card = wait(lambda: find("cv_ap_card") is not None and COMMAND in (text("cv_ap_cmd") or "") + (text("cv_ap_title") or "")
                + (text("cv_ap_body") or ""), 150, 1.0)
    check("X: the approval card shows for the rm -rf (ask mode)", card,
          f"{text('cv_ap_title')!r} {text('cv_ap_cmd')!r}")
    capture("x-approval")
    if not card:
        return x
    # ---- Session Y: New chat.
    sidebar_open()
    check("CLICK New chat -> Session Y", click("sb_new_chat_hit"))
    y = None
    if wait(lambda: any(b.get("session_id") != x for b in wire("session/open")), 10):
        y = next(b.get("session_id") for b in wire("session/open") if b.get("session_id") != x)
    check("Session Y opened (session/open of a new id)", bool(y), str(y))
    sidebar_close()
    check("Y: no approval card (X's approval is not Y's)",
          wait(lambda: find("cv_ap_card") is None and find("i0_composer_0") is not None, 10))
    responds_before = len(wire("approval/respond"))
    click("i0_composer_0")
    for k in ("y", "s", "n"):
        key(k)
    time.sleep(2.0)
    check("Y: Y / S / N pressed in Y send nothing (no approval/respond in the trace)",
          len(wire("approval/respond")) == responds_before, str(wire("approval/respond")))
    sidebar_open()
    sn = snap()
    rows = [w for w in sn if w.get("i") == "sb_r_title" and shown(w)]
    waiting = [w.get("t") for w in rows if row_waits(w, sn)]
    check("sidebar: exactly one row waits — X's (the rm -rf conversation)", len(waiting) == 1, str(waiting))
    capture("y-no-card-x-waits")
    # ---- Back on X.
    x_row = next((w for w in rows if row_waits(w, sn)), None)
    opens_x = len(wire("session/open", x))
    check("CLICK X's row", bool(x_row) and (click_w(x_row) or True))
    sidebar_close()
    check("wire: session/open X again", wait(lambda: len(wire("session/open", x)) > opens_x, 10))
    check("wire: X's parked interactions read (session/hydrate include pending_approvals)",
          wait(lambda: any(b.get("include") == ["pending_approvals"] for b in wire("session/hydrate", x)), 10))
    again = wait(lambda: find("cv_ap_card") is not None and COMMAND in ((text("cv_ap_cmd") or "") + (text("cv_ap_body") or "")
                                                                       + (text("cv_ap_title") or "")), 15)
    check("X: its approval card again (restored, X's own)", again)
    capture("x-approval-again")
    check("CLICK Approve once", click("cv_ap_once"))
    ok = wait(lambda: len(wire("approval/respond")) == responds_before + 1, 10)
    resp = wire("approval/respond")[-1] if wire("approval/respond") else {}
    check("wire: one approval/respond, to X's exact Session", ok and resp.get("session_id") == x
          and resp.get("decision") == "approve", json.dumps(resp))
    check("the turn completes", wait(lambda: find("cv_ap_card") is None and find("composer_send_icon") is not None, 150, 1.0))
    check("Approve once ran it: scratch_dir is gone on disk", wait(lambda: not os.path.exists(scratch), 20))
    check("wire: no approval/respond was ever sent with Y's id", not any(b.get("session_id") == y for b in wire("approval/respond")))
    capture("x-approved")
    stop_app()
    return y


def phase_247(y):
    link_ws = os.path.join(LIVE, "link-ws")
    tuple_ = json.dumps([link_ws, "dsflash", y], separators=(",", ":"))
    link = "octoscode://session?s=" + urllib.parse.quote(tuple_, safe="")
    opens_before, reads_before = len(wire("session/open", y)), len(wire("session/hydrate", y))
    if not check("relaunch with the saved link (link-ws -> ws)", start_app({"OCTOSCODE_SESSION_LINK": link})):
        return
    check("the panel 'Open saved conversation' is offered",
          wait(lambda: text("b3_title") == "Open saved conversation", 30), repr(text("b3_title")))
    check("panel: the workspace the link names", (text("b3_link_workspace") or "").replace("\n", "") == link_ws)
    capture("link-panel")
    check("CLICK Open conversation", click("b3_link_open"))
    refused = wait(lambda: (text("b3_link_error") or "").startswith("Not opened"), 15)
    check("refused on the panel", refused, repr(text("b3_link_error")))
    check("trace: the precondition read the server's attestation (session/list {cwd: link-ws})",
          any(b.get("cwd") == link_ws for b in wire("session/list")))
    time.sleep(1.5)
    check("trace: NO session/open for the linked Session", len(wire("session/open", y)) == opens_before,
          str(wire("session/open", y)[opens_before:]))
    check("trace: NO session/hydrate for the linked Session", len(wire("session/hydrate", y)) == reads_before,
          str(wire("session/hydrate", y)[reads_before:]))
    capture("link-refused")
    check("CLICK Dismiss link", click("b3_link_dismiss"))
    check("dismissed: the conversation shows", wait(lambda: find("b3_dialog") is None and find("i0_composer_0") is not None, 8))
    stop_app()
    # The exact workspace opens the same Session.
    ws = os.path.join(LIVE, "ws")
    tuple_ = json.dumps([ws, "dsflash", y], separators=(",", ":"))
    link = "octoscode://session?s=" + urllib.parse.quote(tuple_, safe="")
    if not check("relaunch with the exact saved link", start_app({"OCTOSCODE_SESSION_LINK": link})):
        return
    check("panel offered", wait(lambda: text("b3_title") == "Open saved conversation", 30))
    check("CLICK Open conversation (exact workspace)", click("b3_link_open"))
    check("trace: session/open {Y, cwd: ws}", wait(lambda: any(b.get("cwd") == ws for b in wire("session/open", y)), 15))
    check("trace: Y's history read after the accepted open", wait(lambda: len(wire("session/hydrate", y)) > reads_before, 15))
    check("the panel closes", wait(lambda: find("b3_dialog") is None, 10))
    capture("link-opened")
    stop_app()


def main():
    open(os.path.join(OUT, "checks.txt"), "w").close()
    try:
        y = phase_250()
        if y:
            phase_247(y)
    finally:
        stop_app()
    ok = sum(1 for _, p in RESULTS if p)
    print(f"== {ok}/{len(RESULTS)} checks passed", flush=True)
    with open(os.path.join(OUT, "checks.txt"), "a") as f:
        f.write(f"== {ok}/{len(RESULTS)} checks passed\n")
    return 0 if ok == len(RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
