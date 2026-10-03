#!/usr/bin/env python3
"""A22 — four Session rows walked by CLICKS in the real hidden app against the
scripted `a22_serve` (octos a6ea8505 shapes; no model, no machine paths).

The walk owns both processes and always stops them; every app store lives in
a per-run dir (brief §8). The wire is checked from the server's own log
(`a22_serve --log`: every request, every pushed notification).

  row 228  startup: the sidebar lists the ATTESTED catalog's full Sessions of
           `a22` (+ the Session the app opened) and never another profile's
           row, a bare id or `a22:legacy`; the wire: session/list
           {cwd, profile_id}.
  row 203  CLICK "Review the hydrate path": its history is answered 1.5 s
           late and another client's live turn reaches the socket first —
           the window shows "Loading conversation…", then the history ABOVE
           the live turn, each prompt once, the live answer whole.
  row 216  the effort: Thinking shows High (the open reply's), CLICK Low,
           switch away and back: still Low. Returned prompts: a long turn
           runs, two prompts queue (first with Low, then High is chosen and
           the second queues), the server refuses both — the first comes
           back with ITS effort (Low), and when the composer is emptied the
           second comes back with High.
  row 236  background Sessions: "Build the release" runs a long turn with a
           queued prompt, "Run the test suite" fails, "Pick a branch" asks a
           question — all while "Startup chat" is on screen: the sidebar
           dots read running / failed / waiting; the build's queued prompt
           starts THERE (wire: turn/start for it while another Session is
           selected), then the dot reads done; the Session on screen never
           shows their work.

usage: python3 tools/walk/a22_sessions_walk.py <host-bin> <desktop|phone> [port] [server-port] [out-dir] [--probe]
Exit status 0 when every check passes. Captures: <out-dir>/<mode>-NN-<name>.png (+ .snap.json)
"""
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)
from snapsafe import scrub  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ARGS = [a for a in sys.argv[1:] if not a.startswith("--")]
PROBE = "--probe" in sys.argv
BINARY = ARGS[0] if len(ARGS) > 0 else ""
MODE = ARGS[1] if len(ARGS) > 1 else "desktop"
PORT = int(ARGS[2]) if len(ARGS) > 2 else 8494
SPORT = int(ARGS[3]) if len(ARGS) > 3 else 8496
OUT = os.path.abspath(ARGS[4]) if len(ARGS) > 4 else os.path.join(ROOT, "docs/ux/a22")
PHONE = MODE == "phone"
BASE = f"http://127.0.0.1:{PORT}"
STATE = tempfile.mkdtemp(prefix="a22walk.")
SERVE_LOG = os.path.join(STATE, "serve.jsonl")
TRACE = os.path.join(STATE, "trace.jsonl")
CWD = "/home/user/a22-ws"
RESULTS = []
SHOT_N = [0]


READ_PAUSES = (1.5, 3.0, 6.0)


def get(path, timeout=20):
    """A read is retried after growing pauses (the instrument answers 404 when
    a frame misses its window on a loaded machine); an input (/click, /t, /k,
    /m) never is: its 404 is a coalesced frame, the input was delivered, and a
    click re-sent after a slow frame lands on whatever moved under the
    pointer."""
    once = path.startswith(("/click", "/t?", "/k?", "/m?"))
    for attempt in range(len(READ_PAUSES) + 1):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except Exception:
            if once:
                time.sleep(0.3)
                return b"{}"
            if attempt == len(READ_PAUSES):
                raise
            time.sleep(READ_PAUSES[attempt])


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def is_shown(w):
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def visible(wid, s=None):
    s = s if s is not None else snap()
    return sorted([w for w in s if w.get("i") == wid and is_shown(w)], key=lambda w: (w["r"][1], w["r"][0]))


def rect(wid, s=None, nth=0):
    hits = visible(wid, s)
    return hits[nth]["r"] if len(hits) > nth else None


def text(wid, s=None):
    hits = visible(wid, s)
    return hits[0].get("t", "") if hits else None


def shown(wid, s=None):
    return rect(wid, s) is not None


def texts(s=None):
    s = s if s is not None else snap()
    return [(w.get("i") or "", w.get("t") or "", w["r"]) for w in s if is_shown(w) and w.get("t")]


def wait(pred, secs=10.0, period=0.25):
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
    time.sleep(0.4)


def click(wid, nth=0):
    r = rect(wid, nth=nth)
    if r:
        click_rect(r)
    return r is not None


def key(code):
    get(f"/k?c={code}&wait=1")
    time.sleep(0.2)


def type_text(t):
    """Per key (LESSONS: typing is tested per KEY, not one /t string)."""
    for ch in t:
        get("/t?" + urllib.parse.urlencode({"t": ch, "wait": "1"}))
    time.sleep(0.2)


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    print(f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else ""), flush=True)
    return ok


def shot(name, s=None):
    SHOT_N[0] += 1
    os.makedirs(OUT, exist_ok=True)
    base = os.path.join(OUT, f"{MODE}-{SHOT_N[0]:02d}-{name}")
    with open(base + ".png", "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", base + ".png", "--out", base + ".png"], capture_output=True)
    snapshot = s if s is not None else snap()
    with open(base + ".snap.json", "w") as f:
        json.dump(scrub(snapshot), f, ensure_ascii=False)
    print(f"  shot {os.path.relpath(base, ROOT)}.png", flush=True)


def wire():
    out = []
    try:
        for l in open(SERVE_LOG):
            try:
                out.append(json.loads(l))
            except ValueError:
                pass
    except FileNotFoundError:
        pass
    return out


def requests(method, session=None):
    return [w["params"] for w in wire() if w.get("method") == method
            and (session is None or w.get("params", {}).get("session_id") == session)]


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
        "OCTOSCODE_CONNECTION_FILE": "connection-v1.json",
    }.items():
        e[k] = os.path.join(STATE, v)
    for d in ("cred", "downloads", "recents"):
        os.makedirs(os.path.join(STATE, d), exist_ok=True)
    e.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{SPORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "a22",
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


def probe():
    check("app up against a22_serve", start_app())
    time.sleep(4)
    s = snap()
    os.makedirs(os.path.join(ROOT, "tmp"), exist_ok=True)
    with open(os.path.join(ROOT, "tmp", f"a22-probe-{MODE}.snap.json"), "w") as f:
        json.dump(scrub(s), f, ensure_ascii=False, indent=0)
    with open(os.path.join(ROOT, "tmp", f"a22-probe-{MODE}.png"), "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    for i, t, r in texts(s):
        print(f"  {i!r:40} {t[:60]!r:64} {r}")


CATALOG = ["Startup chat", "Review the hydrate path", "Build the release", "Run the test suite", "Pick a branch",
           "History hiccup"]
NEVER = ["Foreign profile row", "Bare id row", "Legacy row"]
HISTORY = "a22:api:history"
HICCUP = "a22:api:hiccup"
BUILD = "a22:api:build"


def transcript_order(kind):
    """The transcript's `kind` rows top to bottom: read from the top, then
    scrolled down step by step (the list lays out only visible rows). Also
    returns the texts seen twice in one view (a drawn duplicate)."""
    x, y = (690, 330) if not PHONE else (180, 420)
    for _ in range(6):
        get(f"/m?k=scroll&x={x}&y={y}&dy=-900&wait=1")
    order, twice = [], []
    for _ in range(8):
        view = [t for _, t in sorted((r[1], t) for i, t, r in texts() if kind in i)]
        twice += [t for t in set(view) if view.count(t) > 1]
        for t in view:
            if t not in order:
                order.append(t)
        get(f"/m?k=scroll&x={x}&y={y}&dy=240&wait=1")
    return order, sorted(set(twice))


def open_sidebar():
    """Phone: the sessions live in the drawer."""
    if PHONE and not shown("sb_new_chat_hit"):
        click("sidebar_toggle_hit")
        wait(lambda: shown("sb_new_chat_hit"), 4)


def close_sidebar():
    if PHONE and shown("sb_new_chat_hit"):
        click("drawer_close") or key("Escape")
        wait(lambda: not shown("sb_new_chat_hit"), 4)


def sidebar_rows(s=None):
    """[(title, row rect, status)] for the drawn session rows, top to bottom.
    status: the visible dot inside the row (run / wait / done / fail / idle / None)."""
    s = s if s is not None else snap()
    rows = []
    for w in visible("sb_r_open", s):
        r = w["r"]
        title = next((x.get("t") for x in visible("sb_r_title", s) if inside(x["r"], r, 2.0)), None)
        st = None
        for name in ("run", "wait", "done", "fail", "idle"):
            if any(inside(x["r"], r, 2.0) for x in visible(f"sb_st_{name}", s)):
                st = name
        rows.append((title, r, st))
    return rows


def row_of(title, s=None):
    return next(((t, r, st) for t, r, st in sidebar_rows(s) if t == title), None)


def open_row(title):
    open_sidebar()
    hit = row_of(title)
    if not hit:
        return False
    click_rect(hit[1])
    return wait(lambda: text("hd_title") == title if not PHONE else not shown("sb_new_chat_hit"), 8)


def composer_text(s=None):
    hits = visible("i0_composer_0", s)
    return (hits[0].get("val") or "") if hits else None


def send(prompt):
    click("i0_composer_0")
    type_text(prompt)
    key("Return")
    time.sleep(0.6)


def clear_composer():
    click("i0_composer_0")
    n = len(composer_text() or "")
    for _ in range(n + 2):
        key("Backspace")
    time.sleep(0.3)


def starts(text_part=None, session=None):
    return [p for p in requests("turn/start", session) if text_part is None or text_part in (p.get("input") or [{}])[0].get("text", "")]


def segment_on(seg_id):
    """The Thinking segment's fill, sampled from the capture at its laid-out
    rect: the selected pill is black (board-3 screen 6), the others clear."""
    import io
    try:
        from PIL import Image
    except ImportError:
        return None
    s = snap()
    r = rect(seg_id, s)
    win = rect("main_window", s)
    if not r or not win:
        return None
    img = Image.open(io.BytesIO(get("/g?raw=1", timeout=30))).convert("RGB")
    # The capture is the whole shell window; /snap rects are in its points.
    scale = img.width / float(win[2])
    # Sample a few points inside the pill, off the label's glyphs.
    pts = [(r[0] + 6, r[1] + r[3] / 2), (r[0] + r[2] - 6, r[1] + r[3] / 2), (r[0] + r[2] / 2, r[1] + 4)]
    dark = 0
    for x, y in pts:
        px = img.getpixel((int(x * scale), int(y * scale)))
        if sum(px) / 3 < 80:
            dark += 1
    return dark >= 2


def open_thinking():
    click("i0_composer_0")
    type_text("/thinking")
    key("Return")
    return wait(lambda: shown("b3_think_effort_0"), 6)


def close_dialog():
    click("b3_close") or key("Escape")
    wait(lambda: not shown("b3_think_effort_0"), 4)


# ------------------------------------------------------------------ the walk
def phase_228():
    check("app up against a22_serve", start_app())
    open_sidebar()
    check("228: the catalog rows are drawn", wait(lambda: len([t for t, _, _ in sidebar_rows()]) >= 5, 10))
    s = snap()
    titles = [t for t, _, _ in sidebar_rows(s)]
    check("228: exactly the full Sessions of a22 (+ the opened one)", sorted(titles) == sorted(CATALOG), str(titles))
    check("228: no other profile's row, no bare id, no legacy id",
          not any(n in [t for _, t, _ in texts(s)] for n in NEVER), str([t for _, t, _ in texts(s) if t in NEVER]))
    scoped = [p for p in requests("session/list") if p.get("cwd") == CWD and p.get("profile_id") == "a22"]
    check("228 wire: the catalog is read {cwd, profile_id}", bool(scoped), str(requests("session/list")[:3]))
    rows = sidebar_rows(s)
    check("228 layout: every row title inside its row",
          all(r and any(inside(x["r"], r, 2.0) for x in visible("sb_r_title", s)) for _, r, _ in rows))
    check("228 layout: rows >= 28 px tall", all(r[3] >= 28 for _, r, _ in rows), str([r[3] for _, r, _ in rows]))
    shot("228-catalog", s)
    close_sidebar()


def phase_203():
    open_sidebar()
    hit = row_of("Review the hydrate path")
    check("203: CLICK the 'Review the hydrate path' row", bool(hit))
    if not hit:
        return
    click_rect(hit[1])
    # While its history is held back, the live turn's events are buffered:
    # the window says "Loading conversation…", the live prompt is not drawn.
    loading = wait(lambda: any("Loading conversation" in t for _, t, _ in texts()), 3, 0.1)
    s = snap()
    live_early = any("Third prompt" in t for _, t, _ in texts(s))
    check("203: 'Loading conversation…' while the history is held", loading)
    check("203: the live turn is NOT drawn before its history (buffered)", not live_early)
    if loading:
        shot("203-loading", s)
    done = wait(lambda: any("arrived after the history" in t for _, t, _ in texts()), 12)
    check("203: the live answer completes after the history", done)
    time.sleep(0.8)
    s = snap()
    if os.environ.get("A22_DEBUG"):
        for i, t, r in texts(s):
            print("   ", repr(i), repr(t[:50]), r)
    order, twice = transcript_order("userbubble")
    check("203: history first, then the live turn — each prompt once",
          [t.split(":")[0] for t in order] == ["First prompt", "Second prompt", "Third prompt"] and not twice,
          f"{order} twice={twice}")
    s = snap()
    answers = [t for i, t, _ in texts(s) if "assistantprose" in i]
    check("203: the replayed answer the history holds is drawn once",
          sum(1 for t in answers if "max-wins" in t) == 1, str(answers))
    log = wire()
    first_push = next((k for k, w in enumerate(log) if w.get("push") and w.get("session") == HISTORY), None)
    reply = next((k for k, w in enumerate(log) if w.get("reply") == "session/hydrate" and w.get("session") == HISTORY), None)
    check("203 wire: the live events went out BEFORE the history reply",
          first_push is not None and reply is not None and first_push < reply, f"push@{first_push} reply@{reply}")
    reads = [p for p in requests("session/hydrate", HISTORY) if p.get("include") == ["messages"]]
    check("203 wire: one open, one history read for the Session",
          len(requests("session/open", HISTORY)) == 1 and len(reads) == 1,
          f"opens {len(requests('session/open', HISTORY))}, history reads {len(reads)}")
    col_lo, col_hi = (335, 1045 - 12) if not PHONE else (12, 360 - 12)
    rows = [r for i, t, r in texts(s) if "userbubble" in i or "assistantprose" in i]
    check("203 layout: transcript rows inside the conversation column",
          all(col_lo <= r[0] and r[0] + r[2] <= col_hi for r in rows), str(rows[:3]))
    shot("203-history-then-live", s)


def phase_216():
    check("216: back to 'Startup chat' (CLICK)", open_row("Startup chat"))
    time.sleep(1.0)
    check("216: /thinking opens Thinking effort", open_thinking())
    check("216: the open reply's effort is shown (High)", wait(lambda: segment_on("b3_think_effort_2") is True, 4))
    click("b3_think_effort_0")
    check("216: CLICK Low", wait(lambda: segment_on("b3_think_effort_0") is True, 4))
    shot("216-thinking-low")
    close_dialog()
    check("216: switch away (CLICK 'Review the hydrate path')", open_row("Review the hydrate path"))
    time.sleep(0.8)
    check("216: and back (CLICK 'Startup chat')", open_row("Startup chat"))
    time.sleep(1.0)
    n_open = len(requests("session/open", "a22:main"))
    check("216 wire: the Session was re-opened (its reply names High again)", n_open >= 2, str(n_open))
    check("216: /thinking again", open_thinking())
    kept = wait(lambda: segment_on("b3_think_effort_0") is True, 4)
    check("216: the re-open did not overwrite the person's Low", kept)
    shot("216-thinking-kept-low")
    close_dialog()
    # Returned prompts: a long turn runs, two prompts queue, both refused.
    send("[busy] long task")
    check("216: the long turn runs (Stop shows)", wait(lambda: shown("composer_stop_icon"), 6))
    send("first [refuse]")
    check("216: the first prompt queued", wait(lambda: any("1 queued" in t for _, t, _ in texts()), 4))
    check("216: /thinking while it runs", open_thinking())
    click("b3_think_effort_2")
    check("216: CLICK High", wait(lambda: segment_on("b3_think_effort_2") is True, 4))
    close_dialog()
    send("second [refuse]")
    check("216: the second prompt queued", wait(lambda: any("2 queued" in t for _, t, _ in texts()), 4))
    check("216: both refused when the long turn ends",
          wait(lambda: len(starts("[refuse]")) >= 2, 40), str(len(starts("[refuse]"))))
    sent = starts("[refuse]")
    check("216 wire: each captured its effort at admission",
          [p.get("reasoning_effort") for p in sent[:2]] == ["low", "high"], str([p.get("reasoning_effort") for p in sent[:2]]))
    check("216: the first comes back to the empty composer",
          wait(lambda: composer_text() == "first [refuse]", 6), repr(composer_text()))
    shot("216-first-returned")
    key("Return")  # send it again: it carries ITS effort (Low), not the dialog's High
    check("216 wire: the returned first prompt carries its own Low",
          wait(lambda: len(starts("first [refuse]")) >= 2, 6) and starts("first [refuse]")[1].get("reasoning_effort") == "low",
          str([p.get("reasoning_effort") for p in starts("first [refuse]")]))
    check("216: then the second comes back, in order",
          wait(lambda: composer_text() == "second [refuse]", 6), repr(composer_text()))
    key("Return")
    check("216 wire: the returned second prompt carries its own High",
          wait(lambda: len(starts("second [refuse]")) >= 2, 6) and starts("second [refuse]")[1].get("reasoning_effort") == "high",
          str([p.get("reasoning_effort") for p in starts("second [refuse]")]))
    wait(lambda: composer_text() == "first [refuse]", 6)
    clear_composer()


def phase_236():
    check("236: CLICK 'Build the release'", open_row("Build the release"))
    time.sleep(1.0)
    send("[slow] build it")
    check("236: the build runs", wait(lambda: shown("composer_stop_icon"), 6))
    # The status strip reads the window's live turn, which is per Session:
    # the build's own turn shows on the build's strip (the positive control)…
    check("236: the build's status strip shows its own live turn (not 'Ready')",
          wait(lambda: text("b3_strip_state") not in (None, "", "Ready"), 4), repr(text("b3_strip_state")))
    send("and package it")
    check("236: a prompt waits behind it", wait(lambda: any("1 queued" in t for _, t, _ in texts()), 4))
    check("236: CLICK 'Run the test suite'", open_row("Run the test suite"))
    time.sleep(0.8)
    send("[fail] run tests")
    check("236: CLICK 'Pick a branch'", open_row("Pick a branch"))
    time.sleep(0.8)
    send("[ask] which branch")
    check("236: CLICK 'Startup chat'", open_row("Startup chat"))
    open_sidebar()
    states = lambda: {t: st for t, _, st in sidebar_rows()}
    ok = wait(lambda: states().get("Build the release") == "run" and states().get("Run the test suite") == "fail"
              and states().get("Pick a branch") == "wait", 10)
    s = snap()
    st = {t: x for t, _, x in sidebar_rows(s)}
    check("236: background dots — build running, tests failed, branch waiting", ok, str(st))
    check("236: the Session on screen is not live for their work",
          shown("composer_send_icon", s) and not shown("composer_stop_icon", s))
    # …and never on another Session's strip while it runs in the background.
    check("236: the status strip on screen reads 'Ready' — never the build's live turn",
          text("b3_strip_state", s) == "Ready", repr(text("b3_strip_state", s)))
    check("236: the Session on screen is 'Startup chat'", text("hd_title", s) == "Startup chat" or PHONE, repr(text("hd_title", s)))
    shot("236-background-grouped", s)
    # Board 2 screen 2 is the flat ("All") list.
    if click("sg_text_off"):
        time.sleep(0.6)
        shot("236-background-flat")
    # The build ends in the background: its queued prompt starts THERE.
    started = wait(lambda: bool(starts("and package it", BUILD)), 30)
    check("236 wire: the queued prompt started in its own Session while another is selected", started,
          str([p.get("session_id") for p in starts("and package it")]))
    done = wait(lambda: states().get("Build the release") == "done", 12)
    s = snap()
    check("236: the build reads done (completed in background)", done, str({t: x for t, _, x in sidebar_rows(s)}))
    check("236: tests still failed, branch still waiting",
          {t: x for t, _, x in sidebar_rows(s)}.get("Run the test suite") == "fail"
          and {t: x for t, _, x in sidebar_rows(s)}.get("Pick a branch") == "wait")
    for t, r, x in sidebar_rows(s):
        dot = next((w["r"] for name in ("run", "wait", "done", "fail", "idle") for w in visible(f"sb_st_{name}", s) if inside(w["r"], r, 2.0)), None)
        ttl = next((w["r"] for w in visible("sb_r_title", s) if inside(w["r"], r, 2.0)), None)
        if dot and ttl:
            check(f"236 layout: '{t}' dot left of its title, centred on the row",
                  dot[0] + dot[2] <= ttl[0] + 1 and abs((dot[1] + dot[3] / 2) - (r[1] + r[3] / 2)) <= 2.5, f"dot {dot} title {ttl} row {r}")
    shot("236-background-done", s)
    if click("sg_text_on"):
        time.sleep(0.4)
    close_sidebar()


def history_reads(session):
    """The transport's own history reads of `session` (not the parked-interaction read)."""
    return [p for p in requests("session/hydrate", session) if "pending_approvals" not in (p.get("include") or [])]


def click_row(title):
    """CLICK `title`'s sidebar row, whatever is on screen (the row may be the selected one)."""
    open_sidebar()
    hit = row_of(title)
    if not hit:
        return False
    click_rect(hit[1])
    time.sleep(0.6)
    close_sidebar()
    return True


def phase_reopen():
    """'Reopen it from the sidebar to try again' — A19b's notice — on the row that is ALREADY selected."""
    n_open, n_read = len(requests("session/open", HICCUP)), len(history_reads(HICCUP))
    check("reopen: CLICK 'History hiccup'", open_row("History hiccup"))
    close_sidebar()
    failed = wait(lambda: text("history_title") == "Session recovery required"
                  and "history store busy" in (text("history_hint") or ""), 8)
    s = snap()
    check("reopen: its history could not be read — the notice says so, with the server's reason", failed,
          repr(text("history_hint", s)))
    shot("reopen-failed", s)
    open_sidebar()
    hit = row_of("History hiccup")
    check("reopen: its row is the selected one",
          hit is not None and any(inside(x["r"], hit[1], 2.0) for x in visible("sb_r_sel")))
    check("reopen: CLICK the selected row", click_row("History hiccup"))
    check("reopen wire: the Session is opened again", wait(lambda: len(requests("session/open", HICCUP)) == n_open + 2, 8),
          str(len(requests("session/open", HICCUP)) - n_open))
    check("reopen wire: and its history read again", wait(lambda: len(history_reads(HICCUP)) == n_read + 2, 8),
          str(len(history_reads(HICCUP)) - n_read))
    restated = wait(lambda: text("history_title") == "Session recovery required"
                    and "history store still busy" in (text("history_hint") or ""), 8)
    s = snap()
    check("reopen: that read is refused too — the notice re-states, with the new reason", restated,
          repr(text("history_hint", s)))
    shot("reopen-restated", s)
    check("reopen: CLICK the selected row again", click_row("History hiccup"))
    check("reopen wire: read a third time", wait(lambda: len(history_reads(HICCUP)) == n_read + 3, 8),
          str(len(history_reads(HICCUP)) - n_read))
    arrived = wait(lambda: any("reading it again worked" in t for _, t, _ in texts()) and not shown("history_title"), 8)
    s = snap()
    check("reopen: answered — the history arrives, the notice is gone", arrived,
          str([t for _, t, _ in texts(s) if "history" in t.lower()][:4]))
    shot("reopen-history", s)
    # With its history on screen, a CLICK on the selected row does nothing (#34a row 190).
    opens = len(requests("session/open", HICCUP))
    check("reopen: CLICK the selected row once its history is shown", click_row("History hiccup"))
    time.sleep(1.0)
    check("reopen wire: no re-open of a Session whose history is shown", len(requests("session/open", HICCUP)) == opens,
          str(len(requests("session/open", HICCUP)) - opens))


def walk():
    phase_228()
    phase_203()
    phase_216()
    phase_236()
    phase_reopen()
    stop_app()


def main():
    serve = subprocess.Popen([os.path.join(ROOT, "target/debug/examples/a22_serve"), str(SPORT), "--log", SERVE_LOG],
                             stdout=open(os.path.join(STATE, "serve.out"), "w"), stderr=subprocess.STDOUT)
    time.sleep(1.0)
    code = 1
    try:
        if serve.poll() is not None:
            check("a22_serve up (port free?)", False)
        elif PROBE:
            probe()
            code = 0
        else:
            walk()
            failed = [r for r in RESULTS if not r[1]]
            print(f"== WALK a22 sessions {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed", flush=True)
            print(f"   state dir (serve log, trace): {STATE}", flush=True)
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
    import urllib.parse  # noqa: E402
    sys.exit(main())
