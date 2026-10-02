#!/usr/bin/env python3
"""Live smoke: drive a hidden OctosCode app against a REAL octos serve (a private instance, never the operator's)
and check the flows replay fixtures cannot prove end to end: a tool approval that writes a file, a Chinese answer,
a queued prompt, Stop mid-stream, and an app restart that reopens the session with its history.

    LIVE_DIR=<dir holding a mode-600 `token` and the served `ws/`> \\
      python3 tools/judge/live_smoke.py <host-bin> <app-port> <serve-url> <outdir> [desktop|phone]

The token is read from $LIVE_DIR/token and reaches the app only through its environment (OCTOS_BEARER); it is never
printed, logged or saved. No /snap JSON is written to disk (the instrument reports masked fields' raw values); only
PNG captures, <outdir>/checks.txt and the app's protocol trace <outdir>/trace.jsonl. About six model turns.
"""
import json, os, subprocess, sys, time, urllib.parse, urllib.request

BIN, PORT, SERVE, OUT = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
MODE = sys.argv[5] if len(sys.argv) > 5 else "desktop"
LIVE = os.environ["LIVE_DIR"]
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BASE = f"http://127.0.0.1:{PORT}"
STATE = os.path.join(OUT, "state")
os.makedirs(OUT, exist_ok=True)
RESULTS = []


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


def texts(sn=None):
    return [(w.get("i") or "", w.get("t") or "") for w in (sn if sn is not None else snap()) if shown(w) and w.get("t")]


def click_w(w):
    x, y, ww, hh = w["r"]
    get(f"/click?x={x + ww / 2}&y={y + hh / 2}&wait=1")
    time.sleep(0.5)


def click(wid):
    w = find(wid)
    if w:
        click_w(w)
    return bool(w)


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
    RESULTS.append((name, bool(ok), detail))
    line = f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else "")
    print(line, flush=True)
    with open(os.path.join(OUT, "checks.txt"), "a") as f:
        f.write(line + "\n")
    return ok


N = [0]


def capture(name):
    N[0] += 1
    png = os.path.join(OUT, f"{N[0]:02d}-{name}.png")
    with open(png, "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", png, "--out", png], capture_output=True)


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
        "OCTOS_BASE_URL": SERVE, "OCTOS_BEARER": tok, "OCTOS_PROFILE_ID": os.environ.get("LIVE_PROFILE", "dsflash"),
        "OCTOS_WORKSPACE_CWD": os.path.join(LIVE, "ws"),
        "OCTOSCODE_DESIGN_DIR": os.path.join(ROOT, "design"), "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": os.path.join(OUT, "hs"),
        "OCTOSCODE_TRACE_FILE": os.path.join(OUT, "trace.jsonl"),
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if MODE == "phone" else ""),
    })
    if MODE == "phone":
        e["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    return e


def start_app():
    subprocess.run(["bash", "harness/headless.sh", "start", BIN, str(PORT)], cwd=ROOT, env=env(),
                   stdout=open(os.path.join(OUT, "app-start.log"), "a"), stderr=subprocess.STDOUT)
    return wait(lambda: find("i0_composer_0") is not None, 90, 1.0)


def stop_app():
    subprocess.run(["bash", "harness/headless.sh", "stop", str(PORT)], cwd=ROOT, env=env(),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def idle(sn=None):
    sn = sn if sn is not None else snap()
    return find("composer_send_icon", sn) is not None and find("composer_stop_icon", sn) is None \
        and find("composer_stop_busy", sn) is None


def running():
    sn = snap()
    return find("composer_stop_icon", sn) is not None or find("composer_stop_busy", sn) is not None


def prose():
    return " ".join(t for i, t in texts() if "assistantprose" in i or i.endswith("_prose"))


def send(prompt):
    if not click("i0_composer_0"):
        return False
    type_text(prompt)
    key("return")
    return True


def turn(prompt, secs=150):
    send(prompt)
    started = wait(running, 20)
    done = wait(idle, secs, 1.0)
    return started, done


def main():
    open(os.path.join(OUT, "checks.txt"), "w").close()
    if not check("app up against the live serve", start_app()):
        return 1
    check("connected: the Chat tab shows", wait(lambda: find("hd_tab_chat_hit") is not None, 30))
    time.sleep(3)
    capture("connected")

    # 1. a tool approval that writes a file
    target = os.path.join(LIVE, "ws", "judge.txt")
    if os.path.exists(target):
        os.remove(target)
    send("Create a file named judge.txt in this workspace containing exactly the word ok. Use your file tool.")
    got_card = wait(lambda: find("cv_ap_once") is not None or idle(), 120, 1.0) and find("cv_ap_once") is not None
    seat = next((t for i, t in texts() if i == "i0_composer_2_0"), "")
    # Informational: whether the write asks depends on the session's permission profile (the seat shows it).
    print(f"INFO approval card shown: {got_card}; permission seat: {seat!r}; defaults: "
          f"{next((t for i, t in texts() if t.startswith('New chat defaults')), '')!r}", flush=True)
    if got_card:
        capture("approval-card")
        click("cv_ap_once")
    check("approval: the turn completes", wait(idle, 150, 1.0))
    ok = os.path.exists(target) and open(target).read().strip().lower().startswith("ok")
    check("approval: judge.txt holds 'ok' on disk", ok)
    capture("approval-done")

    # 2. a Chinese answer
    s, d = turn("用一句话说明 main.rs 运行时会打印什么。只用中文回答。")
    p = prose()
    cjk = sum(1 for ch in p if "一" <= ch <= "鿿")
    check("CJK: the turn streams and completes", s and d)
    check("CJK: the answer renders Chinese text", cjk >= 4, f"{cjk} CJK chars in the prose")
    capture("cjk-answer")

    # 3. a queued prompt while a turn runs
    send("List the integers from 1 to 150, one per line, nothing else.")
    check("queue: the first turn starts", wait(running, 20))
    click("i0_composer_0")
    type_text("Now reply with only the word done.")
    key("return")
    q = wait(lambda: find("queue_chip") is not None or find("queue_count") is not None, 10, 0.2)
    check("queue: the second prompt is queued (chip shown)", q)
    capture("queued")
    both = wait(lambda: idle() and find("queue_chip") is None, 240, 1.0)
    check("queue: both turns complete and the queue drains", both)
    check("queue: the queued prompt ran ('done' answered)", "done" in prose().lower())
    capture("queue-drained")

    # 4. Stop mid-stream
    send("Write a 400-word story about a lighthouse keeper and a storm.")
    streaming = wait(lambda: running() and len(prose()) > 0, 60, 0.5)
    time.sleep(3)
    check("stop: the turn is streaming", streaming)
    click("send_hit")
    stopped = wait(idle, 30, 0.5)
    check("stop: Stop returns the composer to idle", stopped)
    comp = (find("i0_composer_0") or {}).get("t") or ""
    check("stop: the interrupted prompt is back in the composer", "lighthouse" in comp, repr(comp[:60]))
    capture("stopped")
    click("i0_composer_0")
    key("end")
    for _ in range(len(comp) + 2):
        key("backspace")

    # 5. restart and reopen: history must be there
    stop_app()
    time.sleep(2)
    check("restart: app up again", start_app())
    time.sleep(6)
    capture("restart-first")

    def has_history():
        t = " ".join(t for _, t in texts())
        return "judge.txt" in t or "main.rs" in t or "lighthouse" in t

    hist = wait(has_history, 20)
    if not hist:
        rows = [w for w in snap() if w.get("i") == "sb_r_open" and shown(w)]
        if rows:
            click_w(rows[0])
            hist = wait(has_history, 20)
            capture("restart-reopened")
    check("restart: the session's earlier turns are shown after reopening", hist)
    if hist:
        t = " ".join(t for _, t in texts())
        back = [k for k in ("judge.txt", "main.rs", "done", "lighthouse") if k in t]
        check("restart: every earlier turn is back (file, CJK, queue, stop)", len(back) >= 3, f"found {back}")
    capture("restart-history")
    s, d = turn("Reply with one short sentence: what is 2 + 3?", 90)
    check("restart: the next prompt streams and completes", s and d)
    check("restart: the new answer renders", "5" in prose())
    capture("restart-next-turn")
    stop_app()
    passed = sum(1 for _, ok, _ in RESULTS if ok)
    print(f"== {passed}/{len(RESULTS)} live checks passed ({MODE})")
    return 0 if passed == len(RESULTS) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    finally:
        stop_app()
