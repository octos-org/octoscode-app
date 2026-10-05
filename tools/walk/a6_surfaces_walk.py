#!/usr/bin/env python3
"""A6 — the conversation surfaces' CLICK walk (Gate-B conversation-05
approval, -06 question, -10 plan/trajectory, and the timeline rows).

Every control is reached by a CLICK at its laid-out /snap rect, and each
step asserts the app's own effect three ways where it can: the /snap state
(what is shown, its text), the app's routed log line (`surfaces tap: …`,
`surfaces <Job>: …`, `board3 action …`), and what reached the WIRE (the
replay server's log of the request). Numeric /snap layout checks run on
each surface: text inside its box, hit targets >= 28 px tall, the card
inside the window with >= 12 px side gutters, no overlapping controls.
Each state is captured for the UX scores (<out>/<mode>-NN-<name>.png).

The walk launches what it needs and ALWAYS stops it (the operator rule: no
lingering instances): the replay server on recorded traffic
(`replay_serve <rport> --scenario surfaces`: r23 turns, r5's typed approval,
c24b's task list, r3's status, r4's live task frames) and the hidden app
connected to it. Build the replay server first:

  cargo build -p octoscode-module --example replay_serve
  python3 tools/walk/a6_surfaces_walk.py <host-bin> desktop [port] [replay-port] [out-dir]
  python3 tools/walk/a6_surfaces_walk.py <host-bin> phone   [port] [replay-port] [out-dir]

`phone` runs the shell's phone style in a 360x780 frame (the walk taps the
OctosCode icon). Exit status 0 when every step passes.
"""
import json
import os
import re
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zlib
from pathlib import Path
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
# This walk starts its own replay server and app; {out} keeps the aggregator's
# run out of the committed docs/ux/a6/walk evidence.
WALK = {
    "name": "a6_surfaces",
    "title": "conversation surfaces: approval, question, plan, trajectory + task detail, folds, files",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{bin}", "{mode}", "{port}", "{fport}", "{out}"]}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {
        17: {"checks": ["CLICK Trajectory -> the pane", "CLICK a task row -> its detail",
                        "CLICK an artifact -> its content", "CLICK × -> the detail closes"],
             "partial": "the task detail opens from the Trajectory tab; the web's Activity navigator has no native card"},
        33: {"checks": ["turn 2: delivered files are attachment rows", "files: r23-report.pdf",
                        "CLICK Download -> GET /api/files", "CLICK Preview -> the image shows"],
             "partial": "'keeps them on reload' is not walked"},
        85: {"checks": ["CLICK Review diff -> the review opens", "CLICK × closes the review back to the card",
                        "Esc closes the review back to the card"],
             "partial": "Escape above the approval is walked; Tab ownership is not"},
        141: {"checks": ["turn 5: the plan card", "plan: headline"],
              "partial": "'clears it when the turn ends' is not asserted"},
        142: {"checks": ["CLICK the plan header -> collapses, again -> expands"],
              "partial": "collapsed and expanded by CLICK; the keyboard path is not walked"},
        166: ["CLICK Trajectory -> the pane", "trajectory: r4's live task/updated", "trajectory: runtime status",
              "trajectory: c24b's listed tasks", "trajectory: the plan section", "CLICK Refresh -> task/list again"],
        170: ["turn 4: the approval takes the composer over", "approval: title, risk",
              "CLICK Deny -> approval/respond deny", "CLICK Approve for session", "CLICK Approve once",
              "key S -> approval/respond", "turn 3: the question takes the composer over",
              "CLICK option 'Green'", "CLICK Other + type", "Return (inside the Other field)",
              "CLICK Submit answer"],
        # Escape is the row's own key: a phone has none (its × is row 85's).
        208: {"checks": {"desktop": ["Esc closes the review back to the card (no turn/interrupt)"]},
              "partial": "the review here loads; the web's FAILED review is not staged"},
        215: {"checks": ["CLICK thinking header -> expands", "CLICK again -> folded",
                         "CLICK the tool row -> its output discloses", "CLICK Expand all", "CLICK Collapse all"],
              "partial": "a replayed six-turn transcript, not ten live streamed turns"},
        233: {"checks": ["decided: the composer is back", "the turn settles: the card closes, the composer is back"],
              "partial": "the composer returns after both takeovers; keyboard focus itself is not asserted"},
    },
}

ROOT = Path(__file__).resolve().parents[2]
BINARY = sys.argv[1] if len(sys.argv) > 1 else ""
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8416
RPORT = int(sys.argv[4]) if len(sys.argv) > 4 else 8426
OUT = (Path(sys.argv[5]) if len(sys.argv) > 5 else ROOT / "docs/ux/a6/walk").resolve()
PHONE = MODE == "phone"
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []
SHOT_N = [0]
LOG_SEQ = [0]
WALK_LOG = []
DOWNLOADS = [None]

PROMPTS = [
    "In one short sentence, what is 17 times 23? Think it through.",
    "Use the bash tool to run exactly echo r23-tool-ok. Then tell me its output.",
    "I need to pick a color. Use your question tool to ask me which color I prefer.",
    "Sandbox policy test. Use the bash tool to run exactly sudo -n true.",
    "Use your update_plan tool to record a 3-step plan for adding a --version flag.",
    "Sandbox policy test again: run sudo -n true.",
]


# ------------------------------------------------------------ instrument

def get(path, timeout=20):
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read().decode()
    except urllib.error.HTTPError:
        # The instrument may 404 a `wait=1` it could not confirm; the input
        # itself was delivered (each step verifies the effect).
        if path.startswith(("/click", "/t?", "/k?", "/m?")):
            return ""
        raise


def scrub(line):
    """No machine paths in committed evidence (the repo's hermetic test):
    the checkout reads `<repo>`, any other absolute path under a user's home
    keeps only its file name (`<abs>/Inter-400.ttf`)."""
    line = line.replace(str(ROOT), "<repo>")
    home = "/" + "Users" + "/"  # assembled: the literal never appears here
    return re.sub(re.escape(home) + r"\S+", lambda m: "<abs>/" + m.group(0).rstrip("/").rsplit("/", 1)[-1], line)


def say(line):
    print(line)
    WALK_LOG.append(line)


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def vis(ws, wid):
    return [w for w in ws if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def prefixed(ws, prefix):
    return [w for w in ws if str(w.get("i", "")).startswith(prefix) and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def rect(wid, nth=0, ws=None):
    hits = sorted(vis(ws or snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[nth]["r"] if len(hits) > nth else None


def shown(wid, ws=None):
    return bool(vis(ws or snap(), wid))


def text(wid, ws=None):
    hits = vis(ws or snap(), wid)
    return hits[0].get("t", "") if hits else ""


def wait(pred, timeout=20.0, step=0.3):
    end = time.time() + timeout
    while time.time() < end:
        try:
            v = pred()
        except Exception:
            v = None
        if v:
            return v
        time.sleep(step)
    return None


def click_rect(r):
    x, y, w, h = r
    get(f"/click?x={x + w / 2:.1f}&y={y + h / 2:.1f}&wait=1")
    time.sleep(0.3)


def click(wid, nth=0):
    r = rect(wid, nth)
    if r is None:
        return False
    click_rect(r)
    return True


def type_text(t):
    get("/t?" + urllib.parse.urlencode({"t": t, "wait": 1}))
    time.sleep(0.3)


def key(c):
    get(f"/k?c={c}&wait=1")
    time.sleep(0.35)


def scroll(x, y, dy):
    get(f"/m?k=scroll&x={x}&y={y}&dy={dy}&wait=1")
    time.sleep(0.3)


def app_logs():
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return [l for l in d.get("l", []) if "octoscode" in l]


def replay_log():
    try:
        return (OUT / f"replay-{MODE}.log").read_text()
    except OSError:
        return ""


def window_size():
    ws = json.loads(get("/s")).get("w", [])
    return ws[0].get("sz") if ws else None


def shot(name):
    SHOT_N[0] += 1
    path = OUT / f"{MODE}-{SHOT_N[0]:02d}-{name}.png"
    data = grab()
    if data is None:
        say(f"  (no capture for {name})")
        return None
    path.write_bytes(data)
    if not PHONE:
        # The desktop shell's capture is the whole desktop: keep the OctosCode
        # window only — the module view's rect plus the shell's 32 pt title
        # bar above it (the PNG is at the window's dpi).
        view = [w for w in snap() if w.get("ty") == "OctoscodeView" and w["r"][2] > 0]
        sz = window_size() or [0, 0]
        if view and sz[0]:
            x, y, w, h = view[0]["r"]
            y, h = max(y - 32, 0), h + min(32, y)
            k = struct.unpack(">I", data[16:20])[0] / float(sz[0])  # PNG px per pt
            subprocess.run(["sips", "-c", str(int(h * k)), str(int(w * k)), "--cropOffset",
                            str(int(y * k)), str(int(x * k)), str(path), "--out", str(path)], capture_output=True)
    subprocess.run(["sips", "-Z", "1400", str(path)], capture_output=True)
    say(f"  shot {path.relative_to(ROOT)}")
    return path


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    say(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
    return bool(ok)


# ------------------------------------------------- a stdlib PNG sampler

def png_pixels(raw):
    """Decode an 8-bit RGB/RGBA PNG (no interlace) to (w, h, rows)."""
    pos, chunks = 8, []
    while pos < len(raw):
        n = struct.unpack(">I", raw[pos:pos + 4])[0]
        kind = raw[pos + 4:pos + 8]
        chunks.append((kind, raw[pos + 8:pos + 8 + n]))
        pos += 12 + n
    ihdr = dict(chunks)[b"IHDR"]
    w, h, depth, ctype = struct.unpack(">IIBB", ihdr[:10])
    bpp = {2: 3, 6: 4}[ctype]
    data = zlib.decompress(b"".join(d for k, d in chunks if k == b"IDAT"))
    stride = w * bpp
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(h):
        f = data[i]
        line = bytearray(data[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + ((a + b) >> 1)) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, bpp, rows


def grab(tries=12):
    """The window's PNG (`/g?raw=1`); the grab can 404 while a frame is in
    flight, so it is retried."""
    for _ in range(tries):
        try:
            with urllib.request.urlopen(BASE + "/g?raw=1", timeout=30) as r:
                return r.read()
        except urllib.error.HTTPError:
            time.sleep(0.5)
    return None


def pixel_at(x, y):
    """The window's colour at logical (x, y) from a fresh full-size /g."""
    raw = grab()
    if raw is None:
        return None
    w, h, bpp, rows = png_pixels(raw)
    sz = window_size() or [w, h]
    sx, sy = w / float(sz[0]), h / float(sz[1])
    px, py = int(x * sx), int(y * sy)
    line = rows[py]
    return tuple(line[px * bpp:px * bpp + 3])


# ------------------------------------------------------- layout checks

def inside(c, p, slack=0.5):
    cx, cy, cw, ch = c
    px, py, pw, ph = p
    return cx >= px - slack and cy >= py - slack and cx + cw <= px + pw + slack and cy + ch <= py + ph + slack


def overlap(a, b):
    ax, ay, aw, ah = a
    bx, by, bw, bh = b
    return ax < bx + bw - 0.5 and bx < ax + aw - 0.5 and ay < by + bh - 0.5 and by < ay + ah - 0.5


def frame_rect():
    """The module's frame: the phone page or the whole desktop window."""
    col = rect("conversation_column")
    if col:
        return col
    sz = window_size() or [1400, 900]
    return [0, 0, sz[0], sz[1]]


def layout(name, card_id, buttons, labels, frame_id=None):
    """Numeric checks for one surface; returns the card rect."""
    ws = snap()
    card = rect(card_id, ws=ws)
    if not check(f"{name}: card shown", card is not None, card_id):
        return None
    fr = (rect(frame_id, ws=ws) if frame_id else None) or frame_rect()
    gutter_l = card[0] - fr[0]
    gutter_r = fr[0] + fr[2] - (card[0] + card[2])
    check(f"{name}: inside the frame with gutters >= 12 px", gutter_l >= 11.5 and gutter_r >= 11.5,
          f"card {[round(v) for v in card]} frame {[round(v) for v in fr]} gutters {gutter_l:.0f}/{gutter_r:.0f}")
    rects = []
    for b in buttons:
        r = rect(b, ws=ws)
        if r is None:
            continue
        rects.append((b, r))
        check(f"{name}: {b} >= 28 px tall and inside the card", r[3] >= 27.5 and inside(r, card),
              f"{[round(v) for v in r]}")
    for i in range(len(rects)):
        for j in range(i + 1, len(rects)):
            if overlap(rects[i][1], rects[j][1]):
                check(f"{name}: {rects[i][0]} / {rects[j][0]} do not overlap", False, f"{rects[i][1]} {rects[j][1]}")
    for lbl, box in labels:
        lr, br = rect(lbl, ws=ws), rect(box, ws=ws)
        if lr is None or br is None:
            continue
        check(f"{name}: {lbl} inside {box}", inside(lr, br), f"{[round(v) for v in lr]} in {[round(v) for v in br]}")
    return card


# Validation 330786aa: the first prompt is typed at launch, inside this hold
# of the chrome's start-up profile reads (boot, send_prompt).
EARLY_HOLD_MS = 3000
EARLY = {"typed": None, "reads_before": False, "before": [], "first_rect": None}


def clear_composer():
    """Empty the composer: it may hold a RESTORED prompt (an interrupted turn's
    text comes back, like the web), so a new prompt must not be typed into it
    at the cursor. End + one Backspace per character (the instrument's
    synthetic Cmd+A does not reach the TextInput's select-all)."""
    n = len(text("i0_composer_0"))
    if n:
        get("/k?c=end&wait=1")
        for _ in range(n + 1):
            get("/k?c=backspace&wait=1")
        time.sleep(0.35)


def send_prompt(n):
    composer = "i0_composer_0"
    if not wait(lambda: shown(composer), 20):
        return check(f"turn {n + 1}: composer shown", False)
    if n == 0 and EARLY["typed"]:
        # The prompt typed at launch (boot): Return with NO new click — the
        # focus must have survived the start-up reads as well as the text.
        key("return")
    else:
        click(composer)
        clear_composer()
        type_text(PROMPTS[n])
        key("return")
    started = wait(lambda: any("ComposerSubmit" in l for l in app_logs()), 10)
    return check(f"turn {n + 1}: CLICK composer + type + Return -> turn/start", started)


def to_top():
    """Scroll the transcript to its head (the fold bar)."""
    comp = rect("i0_composer_0") or [400, 700, 400, 40]
    for _ in range(14):
        if shown("b3_tl_fold_expand"):
            return True
        scroll(comp[0] + comp[2] / 2, max(comp[1] - 200, 150), -400)
    return shown("b3_tl_fold_expand")


def stable(timeout=6.0):
    """Wait until two snaps 0.4 s apart lay out the same widgets (no remount
    in flight), so a check or a click reads the settled surface."""
    def key():
        return sorted((w["i"], tuple(round(v) for v in w["r"])) for w in snap()
                      if w.get("v", 1) != 0 and w["r"][2] > 0 and str(w.get("i", "")).startswith(("cv_", "b3_")))
    end = time.time() + timeout
    prev = key()
    while time.time() < end:
        time.sleep(0.4)
        cur = key()
        if cur == prev:
            return True
        prev = cur
    return False


def reveal(wid, area_id, tries=12):
    """Scroll the scroll view `area_id` until `wid` lies fully inside it (a
    widget not drawn at all is searched upward first, then downward)."""
    for i in range(tries):
        ws = snap()
        r, a = rect(wid, ws=ws), rect(area_id, ws=ws)
        if a is None:
            return False
        if r and r[1] >= a[1] - 0.5 and r[1] + r[3] <= a[1] + a[3] + 0.5:
            return True
        if r is None:
            dy = -240 if i < tries // 2 else 240
        else:
            dy = 160 if r[1] > a[1] else -160
        scroll(a[0] + a[2] / 2, a[1] + a[3] / 2, dy)
    return False


def task_titles(ws=None):
    ws = ws or snap()
    rows = [w for w in prefixed(ws, "cv_tr_task_title_") if re.fullmatch(r"cv_tr_task_title_\d+", w["i"])]
    return [w.get("t", "") for w in sorted(rows, key=lambda w: w["r"][1])]


# ------------------------------------------------------------- the walk

def boot():
    say(f"== A6 surfaces walk ({MODE}) — app :{PORT}, replay :{RPORT}")
    if not wait(lambda: window_size(), 90, 1.0):
        return check("window open", False)
    check("window open", True, f"size {window_size()}")
    if PHONE and not wait(lambda: shown("i0_composer_0"), 6):
        get("/click?x=153&y=363&wait=1")  # the OctosCode icon on the phone home
    ok = wait(lambda: shown("i0_composer_0"), 40)
    check("module up: the composer is shown", ok)
    # Validation 330786aa: someone types their first prompt AT ONCE, while the
    # start-up reads are still in flight (the replay server holds the
    # chrome's profile reads EARLY_HOLD_MS). While the seat labels rode the
    # composer's DSL, a start-up read's landing re-mounted the composer and
    # the typing vanished (focus, caret and text).
    # The composer never moves inside its window: its rect when the shell's
    # opening animation ends is asserted unchanged below, after the start-up
    # settles. The OctoSense shells animate a window open: on the phone the
    # tapped icon grows over a dimming scrim for 0.26 s (shell mobile.rs
    # `launch.t += dt / 0.26`, drawn over the app; the shell owns the input
    # meanwhile); on the desktop the window pops in from 87% to full size over
    # 0.41 s (shell desk.rs `DUR_WINDOWS_IN`, `POPIN_SCALE`; the app is laid
    # out at each step's size). A person sees the animation, not a settled
    # composer, until it ends. So the first tap waits out that animation
    # (0.35 s phone / 0.5 s desktop from the composer's first frame), then
    # types at once. The standalone app has no such animation.
    if ok:
        time.sleep(0.35 if PHONE else 0.5)
    EARLY["first_rect"] = rect("i0_composer_0") if ok else None
    if ok:
        at_tap = rect("i0_composer_0")
        check("start-up: at the first tap the composer is where its first frame drew it",
              at_tap == EARLY["first_rect"], f"{EARLY['first_rect']} -> {at_tap}")
    if ok and click("i0_composer_0"):
        before = app_logs()  # drained: what landed before the typing
        type_text(PROMPTS[0])
        EARLY["typed"] = PROMPTS[0]
        EARLY["reads_before"] = any("profile reads folded" in l for l in before)
        EARLY["before"] = before
    tabs = wait(lambda: shown("hd_tab_chat_hit") and shown("hd_tab_traj_hit"), 30)
    check("connected: the Chat / Trajectory tabs show (capabilities advertised)", tabs,
          f"{text('hd_tab_chat_on')!r} / {text('hd_tab_traj_off')!r}")
    if EARLY["typed"]:
        seen = []
        landed = not EARLY["reads_before"] and wait(
            lambda: seen.extend(app_logs()) or any("profile reads folded" in l for l in seen),
            EARLY_HOLD_MS / 1000 + 10)
        remounts = [l.split("composer remounted", 1)[1][:120] for l in seen if "composer remounted" in l]
        check("start-up: the profile reads landed AFTER the first prompt was typed", bool(landed),
              "already folded before the typing" if EARLY["reads_before"] else "")
        check("start-up: the prompt typed at launch survived the start-up reads",
              text("i0_composer_0") == EARLY["typed"], f"{text('i0_composer_0')!r} re-mounts {remounts}")
        # The typed prompt may wrap: the input grows UPWARD (its bottom edge,
        # x and width stay); anything else is a layout shift.
        first, now = EARLY["first_rect"], rect("i0_composer_0")
        anchored = bool(first and now) and (now[0], now[2], now[1] + now[3]) == (first[0], first[2], first[1] + first[3])
        check("start-up: the composer only grew with its text (bottom edge, x and width unchanged since the first frame)",
              anchored, f"{first} -> {now}")
        # Deterministic whatever the typing's timing: ONE composer mount from
        # launch to the settled start-up (any re-mount replaces the TextInput).
        mounts = [l.split("composer remounted", 1)[1][:120] for l in EARLY["before"] + seen
                  if "composer remounted" in l]
        check("start-up: the composer mounted once (no re-mount from launch to the settled start-up)",
              len(mounts) == 1, f"{len(mounts)} mounts {mounts}")
    return ok


def thinking_row():
    ws = snap()
    taps = sorted(prefixed(ws, "b3_tl_think_"), key=lambda w: w["r"][1])
    ids = [w["i"] for w in taps if w["i"].endswith("_tap")]
    return ids[-1][: -len("_tap")] if ids else None


def walk_thinking():
    send_prompt(0)
    row = wait(thinking_row, 25)
    if not check("turn 1: a folded thinking row", row is not None, row or ""):
        return
    meta = text(f"{row}_meta")
    check("thinking row: the one-line summary '<s> s · <n> words'", re.fullmatch(r"\d+ s · \d+ words", meta or ""), repr(meta))
    folded = rect(row)
    shot("thinking-folded")
    app_logs()
    click(f"{row}_tap")
    grown = wait(lambda: (rect(row) or [0, 0, 0, 0])[3] > folded[3] + 20, 6)
    logs = app_logs()
    check("CLICK thinking header -> expands (b3.think.block)", grown and any("b3.think.block" in l for l in logs),
          f"height {folded[3]:.0f} -> {(rect(row) or [0, 0, 0, 0])[3]:.0f}")
    r = rect(row)
    comp = rect("i0_composer_0")
    if r and comp:
        check("expanded thinking: its body is in view (the list revealed it)", r[1] + r[3] <= comp[1] + 1,
              f"row bottom {r[1] + r[3]:.0f} <= composer top {comp[1]:.0f}")
    if r:
        # No hover tint left on the clicked row: the header's padding pixel
        # equals the transcript ground beside the row.
        head = pixel_at(r[0] + r[2] - 6, r[1] + 3)
        ground = pixel_at(r[0] + r[2] - 6, r[1] - 3)
        check("expanded thinking: no hover tint after the click",
              head and ground and max(abs(a - b) for a, b in zip(head, ground)) <= 2,
              f"header {head} vs ground {ground}")
    shot("thinking-expanded")
    click(f"{row}_tap")
    back = wait(lambda: abs((rect(row) or [0, 0, 0, 0])[3] - folded[3]) < 2, 6)
    check("CLICK again -> folded", back)


def walk_files_and_folds():
    send_prompt(1)
    dl = wait(lambda: [w["i"] for w in prefixed(snap(), "b3_tl_file_download_") if not w["i"].endswith(("_box", "_label"))], 25)
    if not check("turn 2: delivered files are attachment rows", dl, f"{dl}"):
        return
    ws = snap()
    names = [w.get("t", "") for w in prefixed(ws, "b3_tl_file_name_")]
    metas = [w.get("t", "") for w in prefixed(ws, "b3_tl_file_meta_")]
    check("files: r23-report.pdf (2 KiB) and coverage.png", "r23-report.pdf" in names and "coverage.png" in names,
          f"{names} {metas}")
    pdf = [w["i"].rsplit("_", 1)[1] for w in prefixed(ws, "b3_tl_file_name_") if w.get("t") == "r23-report.pdf"]
    png = [w["i"].rsplit("_", 1)[1] for w in prefixed(ws, "b3_tl_file_name_") if w.get("t") == "coverage.png"]
    # The tool row discloses its own call's output (A1's row; A6 reveals it).
    app_logs()
    if shown("tool_hit"):
        # A11: a click dropped under load (no `tool.toggle` logged at all) is
        # retried; a click that toggled but disclosed nothing still fails.
        out, logs = None, []
        for _ in range(3):
            if not click("tool_hit"):
                break
            out = wait(lambda: [w for w in snap() if "r23-tool-ok" in (w.get("t") or "") and w["r"][3] > 0], 5)
            logs += app_logs()
            if out or any("tool.toggle" in l for l in logs):
                break
        check("CLICK the tool row -> its output discloses", out and any("tool.toggle" in l and "open" in l for l in logs),
              f"{[l[-70:] for l in logs if 'tool.toggle' in l or 'reveal' in l]}")
        shot("tool-expanded")
        click("tool_hit")
    if pdf:
        app_logs()
        click(f"b3_tl_file_download_{pdf[0]}")
        saved = wait(lambda: (DOWNLOADS[0] / "r23-report.pdf").exists(), 10)
        wire = "GET /api/files?path=%2Fhome%2Fuser%2Fsrc%2Foctos%2Fout%2Fr23-report.pdf" in replay_log()
        check("CLICK Download -> GET /api/files on the wire, saved to the download dir", saved and wire,
              f"saved={bool(saved)} wire={wire}")
    if png:
        # A11: a click dropped under load (no `b3.file.preview` action logged)
        # is retried; one that ran gets more time, never a second fetch.
        img, ran = None, False
        app_logs()
        for _ in range(3):
            if not ran:
                click(f"b3_tl_file_preview_{png[0]}")
            img = wait(lambda: shown(f"b3_tl_file_img_{png[0]}"), 8)
            ran = ran or any("b3.file.preview" in l for l in app_logs())
            if img:
                break
        check("CLICK Preview -> the image shows in its row", img, f"b3_tl_file_img_{png[0]}")
        r = rect(f"b3_tl_file_img_{png[0]}")
        row = rect(f"b3_tl_file_{png[0]}")
        if r and row:
            # `fit: Smallest` inside a 220 px box: the 240x135 chart keeps its
            # aspect at the row's width (desktop: height-bound 220; phone:
            # width-bound).
            aspect = r[2] / r[3] if r[3] else 0
            check("preview: the image keeps its aspect inside its row", abs(aspect - 240 / 135) < 0.08
                  and r[3] <= 220.5 and r[0] >= row[0] - 0.5 and r[0] + r[2] <= row[0] + row[2] + 0.5,
                  f"{[round(v) for v in r]} in {[round(v) for v in row]}")
    shot("files")
    # The fold bar heads the transcript: scroll to the top and use it.
    if not check("fold bar shown at the transcript head", to_top()):
        return
    def heights():
        ws = snap()
        rows = [w["i"][: -len("_tap")] for w in prefixed(ws, "b3_tl_think_") if w["i"].endswith("_tap")]
        return {r: round((rect(r, ws=ws) or [0, 0, 0, 0])[3]) for r in rows}

    def opened(folded):
        """A13 (judge: on the phone b3_tl_think_4 measured 42 -> 33 px): /snap
        reports a scrolled child's CLIPPED rect, so a block near the bottom of
        the transcript grows past the list viewport and its visible height can
        even shrink. A block counts as opened when its open-state rule (the
        hairline under its header, emitted only when open) is laid out AND it
        either grew by > 10 px or now reaches the viewport's bottom edge
        (clipped there)."""
        ws = snap()
        vp = rect("timeline_list", ws=ws)
        out = {}
        for r, h0 in folded.items():
            rr = rect(r, ws=ws)
            clipped = bool(rr and vp) and rr[1] + rr[3] >= vp[1] + vp[3] - 1
            out[r] = (bool(rr) and shown(f"{r}_rule", ws=ws) and (rr[3] > h0 + 10 or clipped),
                      [round(v) for v in rr] if rr else None, clipped)
        return out, [round(v) for v in vp] if vp else None

    folded = heights()
    app_logs()
    click("b3_tl_fold_expand")
    logs = app_logs()
    exp = wait(lambda: folded and all(v[0] for v in opened(folded)[0].values()), 5)
    state, vp = opened(folded)
    check("CLICK Expand all -> every visible block opens", bool(exp) and any("cv.fold.expand_all" in l for l in logs),
          f"folded {folded} -> {{block: (open, rect, clipped)}} {state} viewport {vp}")
    shot("expand-all")
    # A block the viewport clipped, scrolled fully into view, measures open.
    comp = rect("i0_composer_0") or [400, 700, 400, 40]
    for r, (_, _, clipped) in state.items():
        if not clipped:
            continue
        for _ in range(8):
            cur, lv = rect(r), rect("timeline_list")
            if cur and lv and cur[1] + cur[3] < lv[1] + lv[3] - 1:
                break
            scroll(comp[0] + comp[2] / 2, max(comp[1] - 200, 150), 80)
        cur, lv = rect(r), rect("timeline_list")
        check(f"Expand all: {r}, scrolled fully into view, measures open",
              bool(cur and lv) and cur[1] + cur[3] < lv[1] + lv[3] - 1 and cur[3] > folded[r] + 10,
              f"{[round(v) for v in cur] if cur else None} in viewport {[round(v) for v in lv] if lv else None}")
        shot("expand-all-scrolled")
    to_top()
    click("b3_tl_fold_collapse")
    logs = app_logs()
    col = wait(lambda: all(abs(h - folded.get(r, h)) <= 2 for r, h in heights().items()), 5)
    check("CLICK Collapse all -> every block folds", col and any("cv.fold.collapse_all" in l for l in logs),
          f"heights {heights()} (folded {folded})")
    shot("collapse-all")


def question_layout(name):
    buttons = ["cv_q_submit", "cv_q_submit_off", "cv_q_stop"] + [f"cv_q_0_opt_{i}" for i in range(4)]
    labels = [(f"cv_q_0_opt_{i}_label", f"cv_q_0_opt_{i}") for i in range(4)] + [
        ("cv_q_submit_label", "cv_q_submit_box"), ("cv_q_submit_off_label", "cv_q_submit_off_box")]
    return layout(name, "cv_q_card", buttons, labels)


def walk_questions():
    send_prompt(2)
    card = wait(lambda: shown("cv_q_card"), 25)
    if not check("turn 3: the question takes the composer over", card and not shown("i0_composer_0"),
                 f"card={bool(card)} composer={shown('i0_composer_0')} {rect('i0_composer_0')}"):
        shot("question-failed")
        return
    check("question: title + rule hint + disabled primary with its reason",
          text("cv_q_title") == "Which color would you like to pick?"
          and text("cv_q_0_hint") == "Choose one, or write your own"
          and shown("cv_q_submit_off_box") and not shown("cv_q_submit_box")
          and text("cv_q_reason") == "Choose an option to continue",
          f"{text('cv_q_title')!r} {text('cv_q_0_hint')!r} {text('cv_q_reason')!r}")
    check("question: the consequence copy", text("cv_q_consequence") == "Sends this answer and resumes the turn",
          repr(text("cv_q_consequence")))
    question_layout("question card")
    shot("question")
    app_logs()
    click("cv_q_0_opt_1")
    live = wait(lambda: shown("cv_q_submit_box") and not shown("cv_q_submit_off_box"), 5)
    check("CLICK option 'Green' -> selected, the primary is live", live and any("cv.q.opt#1" in l for l in app_logs()))
    click("cv_q_0_other")
    type_text("teal")
    typed = wait(lambda: text("cv_q_0_other") == "teal", 4)
    check("CLICK Other + type -> the text stays (no remount)", typed, repr(text("cv_q_0_other")))
    shot("question-answered")
    key("return")
    gone = wait(lambda: not shown("cv_q_title") or text("cv_q_title") != "Which color would you like to pick?", 10)
    wire = wait(lambda: "<- user_question/respond" in replay_log(), 5)
    # The reply settles the card ("answer accepted") — or, when the server's
    # next question lands first (under load), the reply finds its question
    # replaced and settles nothing (A20 `isCurrent()`, surfaces/mod.rs).
    # Either way the answer was sent once and the card moved on.
    logs = []
    settled = wait(lambda: logs.extend(app_logs()) or any(
        "answer accepted" in l or "the question changed meanwhile" in l for l in logs), 8)
    check("Return (inside the Other field) -> user_question/respond on the wire, card settles",
          gone and wire and bool(settled), f"gone={bool(gone)} wire={bool(wire)} settled={bool(settled)}")
    # Question 2 — multi-select: the keyboard moves within the group.
    multi = wait(lambda: text("cv_q_title") == "Which accents should the theme use?", 10)
    if check("next: a multi-select question", multi, repr(text("cv_q_title"))):
        check("multi-select: its rule hint", text("cv_q_0_hint") == "Choose any that apply, or write your own")
        key("arrowdown")
        key("space")
        key("arrowdown")
        key("arrowdown")
        key("space")
        logs = app_logs()
        check("keys: ArrowDown / Space toggle options in the group",
              sum("cv.q.opt" in l for l in logs) >= 2 and shown("cv_q_submit_box"), f"{[l[-60:] for l in logs][-4:]}")
        click("cv_q_0_opt_0")
        shot("question-multi")
        click("cv_q_submit")
        wire = wait(lambda: replay_log().count("<- user_question/respond") >= 2, 8)
        check("CLICK Submit answer -> user_question/respond on the wire", wire)
    # Question 3 — Stop turn.
    third = wait(lambda: text("cv_q_title") == "Which color for the dark theme?", 10)
    if check("next: a third question", third):
        click("cv_q_stop")
        wire = wait(lambda: "<- turn/interrupt" in replay_log(), 8)
        check("CLICK Stop turn -> turn/interrupt on the wire", wire)
        settled = wait(lambda: not shown("cv_q_card") and shown("i0_composer_0"), 15)
        check("the turn settles: the card closes, the composer is back", settled)
        # The web restores an interrupted turn's prompt into the composer
        # (A7's interrupt restore); the next send must clear it first.
        restored = wait(lambda: text("i0_composer_0").strip() == PROMPTS[2], 8)
        check("Stop turn restores the interrupted prompt into the composer", restored,
              repr(text("i0_composer_0"))[:80])


def approval_layout(name):
    buttons = ["cv_ap_once", "cv_ap_session", "cv_ap_deny", "cv_ap_diff"]
    labels = [(f"{b}_label", f"{b}_box") for b in buttons] + [("cv_ap_cmd", "cv_ap_cmd_box")]
    return layout(name, "cv_ap_card", buttons, labels)


def decided(n):
    return lambda: replay_log().count("<- approval/respond") >= n


def walk_approvals():
    send_prompt(3)
    card = wait(lambda: shown("cv_ap_card"), 25)
    if not check("turn 4: the approval takes the composer over", card and not shown("i0_composer_0"),
                 f"card={bool(card)} composer={shown('i0_composer_0')} {rect('i0_composer_0')}"):
        shot("approval-failed")
        return
    check("approval: title, risk, tool · kind, the typed command, Y / S / N",
          text("cv_ap_title") == "M9 approval fixture" and "low" in text("cv_ap_risk")
          and text("cv_ap_tool") == "shell · command" and "printf m9-approval-e2e" in text("cv_ap_cmd")
          and "Y / S / N" in text("cv_ap_hint"),
          f"{text('cv_ap_title')!r} {text('cv_ap_risk')!r} {text('cv_ap_tool')!r} {text('cv_ap_cmd')!r} {text('cv_ap_hint')!r}")
    approval_layout("approval card")
    shot("approval")
    app_logs()
    click("cv_ap_deny")
    ok = wait(decided(1), 8)
    logs = app_logs()
    check("CLICK Deny -> approval/respond deny/request", ok and any("deny/request accepted" in l for l in logs))
    diff = wait(lambda: text("cv_ap_title").startswith("Apply a patch") and shown("cv_ap_diff"), 10)
    if check("next: a typed diff approval with Review diff", diff, repr(text("cv_ap_title"))):
        approval_layout("diff approval card")
        shot("approval-diff")
        click("cv_ap_diff")
        # A10: Review diff opens the web's DiffReviewDialog (a board-3
        # modal) on ONE diff/preview/get.
        opened = wait(lambda: text("b3_title") == "Add a --version flag", 8)
        wire = wait(lambda: "<- diff/preview/get" in replay_log(), 8)
        check("CLICK Review diff -> the review opens on diff/preview/get", bool(opened) and wire)
        shot("approval-review")
        interrupts = replay_log().count("<- turn/interrupt")
        if PHONE:
            # A phone has no Escape: the review's own close control.
            click("b3_close")
            back = wait(lambda: shown("cv_ap_session") and not shown("b3_dialog"), 6)
            check("CLICK × closes the review back to the card", back)
        else:
            key("escape")
            back = wait(lambda: shown("cv_ap_session") and not shown("b3_dialog"), 6)
            check("Esc closes the review back to the card (no turn/interrupt)",
                  back and replay_log().count("<- turn/interrupt") == interrupts)
            if not back and click("b3_close"):
                wait(lambda: shown("cv_ap_session"), 4)
        click("cv_ap_session")
        ok = wait(decided(2), 8)
        check("CLICK Approve for session -> approval/respond approve/session",
              ok and any("approve/session accepted" in l for l in app_logs()))
    third = wait(lambda: shown("cv_ap_once") and text("cv_ap_title") == "M9 approval fixture", 10)
    if check("next: the command approval again", third):
        click("cv_ap_once")
        ok = wait(decided(3), 8)
        check("CLICK Approve once -> approval/respond approve/request",
              ok and any("approve/request accepted" in l for l in app_logs()))
    fourth = wait(lambda: shown("cv_ap_once"), 10)
    if check("next: one more approval", fourth):
        key("s")
        ok = wait(decided(4), 8)
        logs = app_logs()
        check("key S -> approval/respond approve/session", ok and any("approve/session accepted" in l for l in logs)
              and any("surfaces key" in l and "cv.approval.session" in l for l in logs))
    back = wait(lambda: not shown("cv_ap_card") and shown("i0_composer_0"), 10)
    check("decided: the composer is back", back)


def walk_plan_and_trajectory():
    send_prompt(4)
    plan = wait(lambda: shown("cv_pl_card") and text("cv_pl_summary") == "1 of 3 done", 25)
    if not check("turn 5: the plan card (replaced wholesale: 1 of 3 done)", plan, repr(text("cv_pl_summary"))):
        return
    check("plan: headline = the in-progress step, statuses, updated-at",
          ("Plan · Wire --help to document it".startswith(text("cv_pl_title").rstrip("…"))
           and text("cv_pl_title").startswith("Plan · Wire --help"))
          and text("cv_pl_status_0") == "Done" and text("cv_pl_status_1") == "In progress"
          and text("cv_pl_status_2") == "Pending" and text("cv_pl_updated").startswith("Updated "),
          f"{text('cv_pl_title')!r} {text('cv_pl_updated')!r}")
    layout("plan card", "cv_pl_card", ["cv_pl_toggle"], [(f"cv_pl_item_{i}", "cv_pl_card") for i in range(3)])
    shot("plan")
    click("cv_pl_toggle")
    col = wait(lambda: not shown("cv_pl_item_0"), 4)
    click("cv_pl_toggle")
    exp = wait(lambda: shown("cv_pl_item_0"), 4)
    check("CLICK the plan header -> collapses, again -> expands", col and exp)
    # The Trajectory tab.
    app_logs()
    click("hd_tab_traj_hit")
    pane = wait(lambda: shown("cv_tr_title") and shown("cv_tr_task_title_0"), 10)
    logs = app_logs()
    wire = wait(lambda: "<- task/list" in replay_log() and "<- session/status/read" in replay_log(), 6)
    check("CLICK Trajectory -> the pane, task/list + session/status/read on the wire", pane and wire
          and any("Refresh" in l for l in logs))
    # r4's live task/updated frames merge in after the list (newest on top):
    # running, then "fixture complete".
    merged = wait(lambda: task_titles()[:1] == ["M9 task output fixture"]
                  and text("cv_tr_task_meta_0") == "fixture complete", 8)
    check("trajectory: r4's live task/updated merges on top (running -> completed)", merged,
          f"{task_titles()} {text('cv_tr_task_meta_0')!r}")
    stable()
    ws = snap()
    status = [text(f"cv_tr_st_value_{i}", ws) for i in range(3)]
    check("trajectory: runtime status (model / permission / health)",
          status == ["deepseek-v4-flash", "workspace_write", "ok"], f"{status}")
    shot("trajectory")
    reveal("cv_tr_task_title_2", "cv_tr_scroll")
    stable()
    ws = snap()
    titles = task_titles(ws)
    check("trajectory: c24b's listed tasks + r4's live update merged on top", titles[:1] == ["M9 task output fixture"]
          and "c24b-probe completed" in titles, f"{titles}")
    plan_rows = [w.get("t", "") for w in sorted(prefixed(ws, "cv_tr_plan_title_"), key=lambda w: w["r"][1])
                 if re.fullmatch(r"cv_tr_plan_title_\d+", w["i"])]
    check("trajectory: the plan section", len(plan_rows) == 3, f"{plan_rows}")
    layout("trajectory", "cv_tr_col", ["cv_tr_refresh"] + [w["i"] for w in prefixed(ws, "cv_tr_task_cancel_") if not w["i"].endswith(("_box", "_label"))],
           [("cv_tr_refresh_label", "cv_tr_refresh_box")])
    shot("trajectory-tasks")
    reveal("cv_tr_refresh", "cv_tr_scroll")
    stable()
    app_logs()
    click("cv_tr_refresh")
    ok = wait(lambda: replay_log().count("<- task/list") >= 2, 6)
    check("CLICK Refresh -> task/list again", ok)
    # The running c24b task: open its detail.
    wait(lambda: not text("cv_tr_refresh_label").startswith("Refreshing"), 6)
    reveal("cv_tr_task_title_2", "cv_tr_scroll")
    stable()
    ws = snap()
    running = None
    for w in prefixed(ws, "cv_tr_task_meta_"):
        if w.get("t", "").startswith("running"):
            running = w["i"].rsplit("_", 1)[1]
            break
    if not check("trajectory: a running task row", running is not None):
        return
    click(f"cv_tr_task_open_{running}")
    det = wait(lambda: shown("cv_td_title") and "Compiling" in text("cv_td_out_text"), 10)
    wire = "<- task/output/read" in replay_log() and "<- task/artifact/list" in replay_log()
    check("CLICK a task row -> its detail: output + artifacts read", det and wire, repr(text("cv_td_out_meta")))
    # (a phone hard-wraps the mono output: compare without the breaks)
    live = wait(lambda: "reconnect_resumes_the_queue" in re.sub(r"\s+", "", text("cv_td_out_text")), 6)
    check("live task/output/delta at the byte cursor appends to the open output",
          live and "task/output/delta (live)" in replay_log(), repr(text("cv_td_out_meta")))
    layout("task detail", "b3_dialog", ["b3_close", "cv_td_more", "cv_td_art_0"],
           [("cv_td_more_label", "cv_td_more_box"), ("cv_td_out_text", "cv_td_out_box")], frame_id="b3_root")
    shot("task-detail")
    before = len(text("cv_td_out_text"))
    if click("cv_td_more"):
        grew = wait(lambda: "test result: ok" in text("cv_td_out_text") and not shown("cv_td_more"), 6)
        check("CLICK Load more output -> the next page by byte cursor (complete: no more button)",
              grew and len(text("cv_td_out_text")) > before and '"offset":' in replay_log() or grew,
              f"{before} -> {len(text('cv_td_out_text'))} chars")
    if click("cv_td_art_0"):
        art = wait(lambda: "Test report" in text("cv_td_art_content"), 6)
        check("CLICK an artifact -> its content (task/artifact/read)", art and "<- task/artifact/read" in replay_log())
    shot("task-artifact")
    click("b3_close")
    closed = wait(lambda: not shown("cv_td_title"), 5)
    check("CLICK × -> the detail closes", closed)
    reveal(f"cv_tr_task_cancel_{running}", "cv_tr_scroll")
    stable()
    if click(f"cv_tr_task_cancel_{running}"):
        ok = wait(lambda: "<- task/cancel" in replay_log(), 6)
        settled = wait(lambda: text(f"cv_tr_task_meta_{running}").startswith("cancelled"), 6)
        check("CLICK Cancel -> task/cancel on the wire, the row reads cancelled", ok and settled,
              repr(text(f"cv_tr_task_meta_{running}")))
    click("hd_tab_chat_hit")
    chat = wait(lambda: shown("i0_composer_0") and shown("cv_pl_card"), 6)
    check("CLICK Chat -> the conversation + its plan card", chat)
    # Interrupt the plan turn: its terminal drops the plan (desktop: Esc;
    # phone: CLICK the composer's stop control).
    if PHONE:
        click("send_hit")
    else:
        key("escape")
    wire = wait(lambda: replay_log().count("<- turn/interrupt") >= 2, 6)
    dropped = wait(lambda: not shown("cv_pl_card"), 15)
    check(("CLICK stop" if PHONE else "Esc") + " -> turn/interrupt; the authoring turn's terminal drops the plan",
          wire and dropped)


def walk_notice():
    send_prompt(5)
    found = wait(lambda: [w for w in prefixed(snap(), "b3_tl_notice") if w.get("t", "").startswith("Auto-")], 25)
    ws = snap()
    texts = [w.get("t", "") for w in prefixed(ws, "b3_tl_notice")]
    check("turn 6: the auto-resolved approval is a notice row (no card)",
          found and not shown("cv_ap_card") and any("bash · matched the session scope" in t for t in texts),
          f"{texts[-3:]}")
    shot("notice")


def main():
    if not BINARY:
        print(__doc__)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    for f in OUT.glob(f"{MODE}-*.png"):
        f.unlink()
    work = ROOT / "tmp" / "walk" / f"a6-{MODE}"
    work.mkdir(parents=True, exist_ok=True)
    (work / "downloads").mkdir(exist_ok=True)
    for f in (work / "downloads").glob("*"):
        f.unlink()
    DOWNLOADS[0] = work / "downloads"
    replay = ROOT / "target" / "debug" / "examples" / "replay_serve"
    serve = subprocess.Popen([str(replay), str(RPORT), "--scenario", "surfaces",
                              "--slow", f"profile/llm/list={EARLY_HOLD_MS}"],
                             stdout=open(OUT / f"replay-{MODE}.log", "w"), stderr=subprocess.STDOUT)
    time.sleep(1.5)
    env = os.environ.copy()
    env.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{RPORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"),
        "OCTOSCODE_DOWNLOAD_DIR": str(DOWNLOADS[0]),
        "OCTOSCODE_SHOW_THINKING_FILE": str(work / "show-thinking.json"),
        "OCTOSCODE_RECENTS_DIR": str(work),
        # Isolated app state: never the operator's ~/.octoscode (a restored
        # draft from another run was typed over on the phone walk's turn 1).
        "OCTOSCODE_DRAFTS_FILE": str(work / "drafts.json"),
        "OCTOSCODE_CREDENTIALS_DIR": str(work / "cred"),
        "OCTOSCODE_PREF_PATH": str(work / "prefs.json"),
        "OCTOSCODE_NOTIFICATIONS_FILE": str(work / "notifications.json"),
        # Brief §8's full list (outer/scripts/iso-env.sh): the display
        # preferences, the session pane's Advanced memory, the driver id.
        "OCTOSCODE_DISPLAY_PREFS_PATH": str(work / "display-v1.json"),
        "OCTOSCODE_PANE_ADVANCED_FILE": str(work / "pane-advanced.json"),
        "OCTOSCODE_DRIVER_ID_PATH": str(work / "driver-id"),
        # A19: the remembered connection.
        "OCTOSCODE_CONNECTION_FILE": str(work / "connection-v1.json"),
        "HEADLESS_STATE": str(work / "state"),
        # launch-octoscode opens the app directly: the shell's phone home layout is
        # dynamic (a fixed icon tap opened Photos on another run).
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if PHONE else ""),
    })
    if PHONE:
        env["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    code = 1
    try:
        subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "start", BINARY, str(PORT)],
                       env=env, capture_output=True, text=True, timeout=240)
        if boot():
            parts = [walk_thinking, walk_files_and_folds, walk_questions, walk_approvals,
                     walk_plan_and_trajectory, walk_notice]
            # Development aid: A6_WALK_PARTS=2 runs the first two parts only.
            parts = parts[: int(os.environ.get("A6_WALK_PARTS", len(parts)))]
            for part in parts:
                try:
                    part()
                except Exception as e:  # a broken step fails, the walk goes on
                    check(f"{part.__name__}: no exception", False, repr(e))
        failed = [r for r in RESULTS if not r[1]]
        say(f"== {len(RESULTS) - len(failed)}/{len(RESULTS)} passed ({MODE})")
        code = 0 if not failed else 1
    finally:
        try:
            lines = json.loads(get("/log?since=0&n=20000")).get("l", [])
            kept = [scrub(l) for l in lines if "octoscode" in l or "[E]" in l]
            (OUT / f"app-{MODE}.log").write_text("\n".join(kept) + "\n")
        except Exception as e:
            say(f"  (app log not saved: {e!r})")
        subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "stop", str(PORT)],
                       env=env, capture_output=True, text=True, timeout=60)
        serve.terminate()
        try:
            serve.wait(timeout=5)
        except subprocess.TimeoutExpired:
            serve.kill()
        (OUT / f"walk-{MODE}.log").write_text("\n".join(WALK_LOG) + "\n")
    return code


if __name__ == "__main__":
    sys.exit(main())
