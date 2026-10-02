#!/usr/bin/env python3
"""A7 — the composer / markdown / history / media / attention click walk.

Every control is reached from the real UI by a CLICK at its laid-out rect
(or a key / text event where the user types), and each step asserts the app's
own effect: a /snap visibility or text, and/or the routed log line. Numeric
/snap checks cover hit targets (>= 28 px), clipping and overlap of the A7
surfaces (queued chip, recovery notice, seat line, read-only peer row, code
block banner).

The app runs hidden with the instrument on PORT against the scripted fixture
server `crates/octoscode-module/examples/a7_serve.rs` (no model, no secrets):

  cargo run -p octoscode-module --example a7_serve -- 8427 &
  OCTOS_BASE_URL=http://127.0.0.1:8427 OCTOS_PROFILE_ID=a7 \\
  OCTOSCODE_DESIGN_DIR=$PWD/design MAKEPAD_WM_TEST_APP=octoscode \\
  OCTOSCODE_TURN_START_TIMEOUT_MS=4000 \\
  OCTOSCODE_DRAFTS_FILE=$PWD/tmp/drafts.json \\
  OCTOSCODE_NOTIFICATIONS_FILE=$PWD/tmp/notifications.json \\
  HEADLESS_ARGS="--module octoscode" bash harness/headless.sh start <host-bin> 8417
  python3 tools/walk/a7_composer_walk.py 8417 desktop main
  bash harness/headless.sh stop 8417

Scenarios (third argument):
  main        markdown (links, math, highlighted code, Copy -> Copied -> Copy),
              shortcut suppression, queue (FIFO chip, Steer now, remove),
              collision notice, recovery (Check status / Continue without it),
              history (/fork, /undo), read-only peer, notification consent,
              images (drop, Upload twice, Cancel, remove in flight).
  seat        needs the fixture started with A7_SERVE_HELD=<driver id>
              (and A7_SERVE_RELEASE_DELAY_MS=1500): a held Session refuses the
              send with no frame, Take over = acquire -> release -> one send.
  draft-save  type a draft and leave it unsent;
  draft-check after a restart of the app: the draft is back, nothing sent.

`phone` mode expects the 360x780 frame (`--test-action page:0`,
OCTOSENSE_WINDOW_SIZE=360x780); the walk opens the app from the phone home.
Optional env: A7_WALK_OUT=<dir> saves a PNG per state; A7_SERVE_LOG=<file>
(the fixture's stdout) lets the image steps count upload requests.
Exit status: 0 when every step passes.
"""
import json
import os
import struct
import sys
import time
import urllib.parse
import urllib.request
import zlib
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
# Four runs on one isolated state: an unsent draft, an app restart that must
# bring it back unsent, the main walk, then the held seat (the fixture
# restarted with A7_SERVE_HELD).
WALK = {
    "name": "a7_composer",
    "title": "composer: markdown/code copy, queue/steer, collision, recovery, history, peer, attention, media, seat, drafts",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/a7_serve", "{fport}"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "a7",
                    "OCTOSCODE_TURN_START_TIMEOUT_MS": "4000"},
            "ready": ["i0_composer_0", "hd_held"]},
    "runs": [
        {"argv": ["{port}", "{mode}", "draft-save"]},
        {"restart": "app", "argv": ["{port}", "{mode}", "draft-check"],
         "env": {"A7_SERVE_LOG": "{fixture_log}"}},
        {"restart": "app", "argv": ["{port}", "{mode}", "main"],
         "env": {"A7_WALK_OUT": "{out}", "A7_SERVE_LOG": "{fixture_log}"}},
        {"restart": "both", "fixture_env": {"A7_SERVE_HELD": "octos-tui", "A7_SERVE_RELEASE_DELAY_MS": "1500"},
         "argv": ["{port}", "{mode}", "seat"], "env": {"A7_WALK_OUT": "{out}", "A7_SERVE_LOG": "{fixture_log}"}},
    ],
    "needs": ["target/debug/examples/a7_serve"],
    "timeout": 900,
    "rows": {
        4: {"checks": ["attention:"],
            "partial": "the opt-in and its persistence; OS notices staying silent while reading are not observable headless"},
        14: ["seat:"],
        15: {"checks": ["collision:"],
             "partial": "the busy refusal keeps the text; a LATE collision against newer text and images is not staged"},
        19: {"checks": ["history: /fork opens", "history: Create is disabled", "history: a valid name arms Create",
                        "history: the typed name stays", "history: the fork opens in the background",
                        "history: the dialog names the child", "history: the sidebar lists the forked conversation"],
             "partial": "the receipt across a parent refresh is not walked"},
        20: {"checks": ["history: /undo opens", "history: Restore asks for confirmation first",
                        "history: Confirm restores the snapshot"],
             "partial": "the receipt through a canonical refresh is not walked"},
        38: {"checks": ["draft:"], "partial": "'sent drafts stay cleared' is not asserted"},
        140: {"checks": ["peer:"], "partial": "'Esc never leaks' is not asserted here"},
        151: ["recovery: an unacknowledged start holds the turn", "recovery: the timeout is disclosed",
              "recovery: Continue without it releases the hold"],
        152: ["recovery: Check status asks turn/state/get and clears the hold",
              "recovery: an unknown lifecycle keeps the turn held", "recovery: the second lost turn is held"],
        190: {"checks": ["recovery:"], "partial": "the queue and a new draft across the unknown turn are not asserted"},
        225: {"checks": ["queue: a prompt sent while a turn runs is queued",
                         "queue: ✕ removes the queued prompt without interrupting"],
              "partial": "queued messages can be removed; IME confirmation and multiline editing are not walked natively"},
    },
}

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8417
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
SCENARIO = sys.argv[3] if len(sys.argv) > 3 else "main"
BASE = f"http://127.0.0.1:{PORT}"
OUT = os.environ.get("A7_WALK_OUT", "")
SERVE_LOG = os.environ.get("A7_SERVE_LOG", "")
RESULTS = []


# ---------------------------------------------------------------- instrument

def get(path, timeout=30):
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read()
    except urllib.error.HTTPError as e:
        return e.read()


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def shown(w):
    r = w.get("r") or [0, 0, 0, 0]
    return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0


def visible(wid, sn=None):
    sn = sn if sn is not None else snap()
    hits = [w for w in sn if w.get("i") == wid and shown(w)]
    return sorted(hits, key=lambda w: (w["r"][1], w["r"][0]))


def visible_prefix(prefix, sn=None):
    sn = sn if sn is not None else snap()
    return [w for w in sn if str(w.get("i") or "").startswith(prefix) and shown(w)]


def is_shown(wid):
    return bool(visible(wid))


def text_of(wid):
    v = visible(wid)
    return v[0].get("t", "") if v else None


def texts_matching(pred, sn=None):
    sn = sn if sn is not None else snap()
    return [w for w in sn if shown(w) and pred(str(w.get("t", "")))]


def rect(wid, nth=0):
    v = visible(wid)
    return v[nth]["r"] if len(v) > nth else None


def click_rect(r):
    x, y, w, h = r
    get(f"/click?x={x + w / 2:.1f}&y={y + h / 2:.1f}&wait=1")
    time.sleep(0.25)


def click(wid, nth=0):
    r = rect(wid, nth)
    if r is None:
        return False
    click_rect(r)
    return True


def type_text(text):
    get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
    time.sleep(0.2)


def key(code, **mods):
    q = "".join(f"&{k}=1" for k, v in mods.items() if v)
    get(f"/k?k=down&c={code}{q}&wait=1")
    get(f"/k?k=up&c={code}{q}&wait=1")
    time.sleep(0.25)


def scroll(dy, x=None, y=None):
    if x is None:
        col = rect("conversation_column") or [360, 120, 600, 400]
        x, y = col[0] + col[2] / 2, col[1] + col[3] / 3
    get(f"/m?k=scroll&x={x:.0f}&y={y:.0f}&dy={dy}&wait=1")
    time.sleep(0.5)


LOG_SEQ = [0]


def log_since():
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return d.get("l", [])


LOG_SEEN = []


def logs():
    LOG_SEEN.extend(log_since())
    return LOG_SEEN


def logged(needle):
    return any(needle in l for l in logs())


def wait(pred, secs=10.0, step=0.25):
    end = time.time() + secs
    while time.time() < end:
        if pred():
            return True
        time.sleep(step)
    return bool(pred())


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
    return bool(ok)


def grab(state):
    """Save the window PNG and its /snap (input values stripped: a
    TextInput's raw value is never written) for the numeric UX checks."""
    if not OUT:
        return
    os.makedirs(OUT, exist_ok=True)
    sn = json.loads(get("/snap?all=1"))
    for w in sn.get("s", []):
        w.pop("val", None)
    json.dump(sn, open(os.path.join(OUT, f"{MODE}-{state}.json"), "w"))
    for _ in range(6):
        data = get("/g?raw=1", timeout=60)
        if data[:4] == b"\x89PNG":
            open(os.path.join(OUT, f"{MODE}-{state}.png"), "wb").write(data)
            return
        time.sleep(0.5)


def hit_ok(wid):
    r = rect(wid)
    return r is not None and r[2] >= 28 and r[3] >= 28, r


def inside(child, parent):
    c, p = rect(child), rect(parent)
    if not c or not p:
        return False
    return c[0] >= p[0] - 0.5 and c[1] >= p[1] - 0.5 and c[0] + c[2] <= p[0] + p[2] + 0.5 and c[1] + c[3] <= p[1] + p[3] + 0.5


def overlaps(a, b):
    return not (a[0] + a[2] <= b[0] or b[0] + b[2] <= a[0] or a[1] + a[3] <= b[1] or b[1] + b[3] <= a[1])


# ------------------------------------------------------------- app helpers

PLACEHOLDER = "Ask Octos anything"


def composer_text():
    v = visible("i0_composer_0")
    if not v:
        return ""
    w = v[0]
    if "val" in w:
        return str(w.get("val") or "")
    t = str(w.get("t") or "")
    return "" if t == PLACEHOLDER else t


def focus_composer():
    return click("i0_composer_0")


def clear_composer():
    focus_composer()
    for _ in range(3):
        key("KeyA", logo=1)
        key("Backspace")
        if composer_text() == "":
            return True
    # Fall back to deleting character by character.
    for _ in range(len(composer_text()) + 2):
        key("Backspace")
    return composer_text() == ""


def send(text):
    focus_composer()
    if composer_text():
        clear_composer()
    type_text(text)
    key("Return")
    time.sleep(0.3)


def turn_idle(secs=20):
    """The composer's send button is back (no live turn)."""
    return wait(lambda: not any("Working" in str(w.get("t", "")) for w in visible_prefix("i0_workingrow")), secs)


def notice_with(title, body_part=None):
    sn = snap()
    titles = [w for w in sn if str(w.get("i", "")).startswith("b3_tl_notice_title") and shown(w) and w.get("t") == title]
    if not titles:
        return False
    if body_part is None:
        return True
    return any(body_part in str(w.get("t", "")) for w in sn
               if str(w.get("i", "")).startswith("b3_tl_notice_body") and shown(w))


def dismiss_keyboard():
    """The phone shell's emulated soft keyboard (drawn by the shell, not in
    /snap) stays up after a dialog field had focus and covers the composer:
    tap its hide chevron (top-right of the keyboard, 360x780 frame)."""
    if MODE == "phone":
        get("/click?x=337&y=513&wait=1")
        time.sleep(0.6)


def open_drawer_if_phone():
    if MODE == "phone" and not is_shown("drawer_scrim"):
        click("sidebar_toggle_hit")
        wait(lambda: is_shown("drawer_scrim"), 3)


def close_drawer_if_phone():
    if MODE == "phone" and is_shown("drawer_scrim"):
        click("drawer_close")
        wait(lambda: not is_shown("drawer_scrim"), 3)


def session_titles():
    return [str(w.get("t", "")) for w in snap() if w.get("i") == "sb_r_title" and shown(w)]


def session_row(title_part):
    for w in snap():
        if w.get("i") == "sb_r_title" and shown(w) and title_part in str(w.get("t", "")):
            return w["r"]
    return None


def open_session(title_part):
    open_drawer_if_phone()
    r = wait(lambda: session_row(title_part) is not None, 8) and session_row(title_part)
    if not r:
        return False
    # The row's open hit spans the row; click its centre line.
    for w in visible("sb_r_open"):
        if overlaps(w["r"], r):
            click_rect(w["r"])
            break
    else:
        click_rect(r)
    time.sleep(1.0)
    close_drawer_if_phone()
    return True


def png(path, rgb=(40, 120, 200), w=8, h=8):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(t, data):
        c = struct.pack(">I", len(data)) + t + data
        return c + struct.pack(">I", zlib.crc32(t + data) & 0xFFFFFFFF)

    data = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "wb").write(data)


def served(needle):
    """How many fixture log lines carry `needle` (None without A7_SERVE_LOG)."""
    if not SERVE_LOG or not os.path.exists(SERVE_LOG):
        return None
    return sum(1 for l in open(SERVE_LOG, errors="replace") if needle in l)


def uploads_seen():
    if not SERVE_LOG or not os.path.exists(SERVE_LOG):
        return None
    lines = open(SERVE_LOG, errors="replace").read().splitlines()
    return [l for l in lines if "POST /api/upload" in l], [l for l in lines if "-> receipt" in l]


# ------------------------------------------------------------------- steps

def markdown_walk():
    send("show the markdown answer")
    ok = wait(lambda: any(w.get("ty") == "A7CodeLines" for w in snap() if shown(w)), 12)
    check("markdown: the answer renders highlighted code (A7CodeLines)", ok)
    turn_idle(15)
    sn = snap()
    math = [w for w in sn if w.get("ty") in ("MathView", "A7MathBlock") and shown(w)]
    check("markdown: inline + display math typeset (MathView)", len(math) >= 2, f"{len(math)}")
    money = texts_matching(lambda t: "$12" in t, sn)
    check("markdown: money stays prose ($12 and $5)", bool(money))
    labels = [w.get("t") for w in sn if str(w.get("i", "")).endswith("_lang") and shown(w)]
    check("markdown: code banners name the language", "toml" in labels or "rust" in labels, f"{labels}")
    grab("md-answer")
    # The answer's head: its links and the (not loaded) image.
    scroll(-900)
    sn = snap()
    links = [w for w in sn if w.get("ty") == "MarkdownLink" and shown(w)]
    check("markdown: only the absolute https link is a link (javascript: stays text)", len(links) == 1,
          f"{len(links)} MarkdownLink")
    check("markdown: the remote image is not loaded (alt text only)",
          not any(w.get("ty") == "Image" for w in sn if shown(w) and str(w.get("i", "")).startswith("i")))
    labels = [w.get("t") for w in sn if str(w.get("i", "")).endswith("_lang") and shown(w)]
    check("markdown: the rust fence is labelled", "rust" in labels, f"{labels}")
    grab("md-answer-top")
    # Copy -> "Copied" for 1 s -> "Copy" (the trimmed code to the clipboard).
    copies = sorted([w for w in snap() if str(w.get("i", "")).startswith("code_copy_") and shown(w)],
                    key=lambda w: w["r"][1])
    if not check("markdown: a code block offers Copy", bool(copies)):
        return
    ok28 = all(w["r"][2] >= 28 and w["r"][3] >= 28 for w in copies)
    check("layout: Copy hit targets >= 28 px", ok28, f"{[w['r'] for w in copies]}")
    target = copies[0]
    k = target["i"].split("_")[-1]
    log_since()
    click_rect(target["r"])
    label = [w for w in snap() if str(w.get("i", "")).endswith(f"_code_{k}_copy_label") and shown(w)]
    check("markdown: Copy shows Copied", bool(label) and label[0].get("t") == "Copied",
          f"{[w.get('t') for w in label]}")
    check("markdown: the copy reached the clipboard (logged)", logged("code.copy"))
    grab("md-copied")
    time.sleep(1.4)
    label = [w for w in snap() if str(w.get("i", "")).endswith(f"_code_{k}_copy_label") and shown(w)]
    check("markdown: Copied returns to Copy after 1 s", bool(label) and label[0].get("t") == "Copy",
          f"{[w.get('t') for w in label]}")


def shortcut_walk():
    # Inside the composer (a text input) the parity chord is suppressed.
    focus_composer()
    log_since()
    key("KeyD", alt=1)
    lines = log_since()
    check("shortcuts: Alt+D inside the composer is suppressed",
          any("suppressed" in l and "target_is_text_input: true" in l for l in lines)
          and not any("Alt+D -> fleet" in l for l in lines))
    # Inside a dialog (the fork dialog): suppressed as well.
    send("/fork")
    if wait(lambda: is_shown("b3_ck_fork_name"), 6):
        log_since()
        key("KeyD", alt=1)
        lines = log_since()
        check("shortcuts: Alt+D inside a dialog is suppressed",
              any("suppressed" in l and "in_dialog: true" in l for l in lines))
        click("b3_close") or key("Escape")
        wait(lambda: not is_shown("b3_ck_fork_name"), 4)


def queue_walk():
    send("slow walk through the queue")
    wait(lambda: logged("turn/start") and visible_prefix("i0_workingrow"), 6)
    time.sleep(1.0)
    send("second prompt")
    ok = wait(lambda: is_shown("queue_chip"), 6)
    check("queue: a prompt sent while a turn runs is queued (chip)", ok)
    check("queue: the chip counts FIFO", text_of("queue_count") == "1 queued", f"{text_of('queue_count')!r}")
    check("queue: the composer cleared on admission", composer_text() == "", repr(composer_text()))
    steer_ok = is_shown("queue_steer_hit")
    check("queue: Steer now is offered (turn/steer advertised, accepted owner running)", steer_ok)
    for wid in ("queue_steer_hit", "queue_remove_hit"):
        ok, r = hit_ok(wid)
        check(f"layout: {wid} >= 28 px", ok, f"{r}")
    chip, comp = rect("queue_chip"), rect("i0_composer")
    check("layout: the chip sits above the composer without overlap",
          bool(chip and comp) and chip[1] + chip[3] <= comp[1] + 0.5, f"chip {chip} composer {comp}")
    grab("queue-chip")
    log_since()
    click("queue_steer_hit")
    check("queue: Steer now steers into the running turn (chip gone)", wait(lambda: not is_shown("queue_chip"), 5))
    check("queue: turn/steer was accepted into the captured turn",
          wait(lambda: any("turn/steer" in l and "true" in l for l in logs()), 5))
    send("third prompt")
    if wait(lambda: is_shown("queue_chip"), 5):
        log_since()
        click("queue_remove_hit")
        check("queue: ✕ removes the queued prompt without interrupting",
              wait(lambda: not is_shown("queue_chip"), 4) and logged("removed: true")
              and not logged("turn/interrupt"))
    else:
        check("queue: a second queued prompt shows the chip", False)
    turn_idle(25)
    check("queue: the steered text is in the answer",
          bool(texts_matching(lambda t: "Steered: second prompt" in t)))
    grab("queue-done")


def steer_command_walk():
    # `/steer on`: the next prompt sent during a turn steers immediately.
    send("/steer on")
    time.sleep(0.6)
    send("slow steering demo")
    wait(lambda: visible_prefix("i0_workingrow"), 6)
    time.sleep(1.0)
    log_since()
    send("steer this directly")
    check("steer: with /steer on a prompt steers instead of queueing",
          wait(lambda: bool(texts_matching(lambda t: "Steered: steer this directly" in t)), 8)
          and not is_shown("queue_chip"))
    turn_idle(25)
    send("/steer off")
    time.sleep(0.6)


def collision_walk():
    send("busy please")
    ok = wait(lambda: notice_with("Session busy", "Another client was working"), 8)
    check("collision: a typed turn_in_progress refusal reads as Session busy", ok)
    check("collision: the text is kept for retry (back in the composer)",
          wait(lambda: composer_text() == "busy please", 4), repr(composer_text()))
    check("collision: never 'Turn rejected'", not notice_with("Turn rejected"))
    grab("collision-notice")
    turn_idle(20)
    clear_composer()


def recovery_walk():
    send("lost turn please")
    ok = wait(lambda: is_shown("recovery_check_hit"), 10)
    check("recovery: an unacknowledged start holds the turn (recovery notice)", ok)
    check("recovery: the timeout is disclosed", notice_with("Turn start timed out"))
    for wid in ("recovery_check_hit", "recovery_continue_hit"):
        ok, r = hit_ok(wid)
        check(f"layout: {wid} >= 28 px", ok, f"{r}")
    grab("recovery-notice")
    log_since()
    click("recovery_check_hit")
    check("recovery: Check status asks turn/state/get and clears the hold",
          wait(lambda: not is_shown("recovery_check_hit"), 8))
    send("lost forever")
    if wait(lambda: is_shown("recovery_continue_hit"), 10):
        click("recovery_check_hit")
        time.sleep(1.5)
        still = is_shown("recovery_continue_hit")
        check("recovery: an unknown lifecycle keeps the turn held", still)
        grab("recovery-unknown")
        click("recovery_continue_hit")
        check("recovery: Continue without it releases the hold",
              wait(lambda: not is_shown("recovery_continue_hit"), 5))
    else:
        check("recovery: the second lost turn is held", False)


def history_walk():
    send("/fork")
    if not check("history: /fork opens the Fork conversation dialog", wait(lambda: is_shown("b3_ck_fork_name"), 6)):
        return
    check("history: Create is disabled until a valid name", is_shown("b3_ck_fork_off") and not is_shown("b3_ck_fork_on"))
    click("b3_ck_fork_name")
    type_text("walk-fork")
    dismiss_keyboard()
    check("history: a valid name arms Create", wait(lambda: is_shown("b3_ck_fork_go"), 3))
    check("history: the typed name stays in the field", "walk-fork" in str(
        (visible("b3_ck_fork_name") or [{}])[0].get("val", (visible("b3_ck_fork_name") or [{}])[0].get("t", ""))),
        repr((visible("b3_ck_fork_name") or [{}])[0].get("t")))
    grab("fork-dialog")
    log_since()
    click("b3_ck_fork_go")
    ok = wait(lambda: "background" in (text_of("b3_ck_notice") or ""), 8)
    check("history: the fork opens in the background (selection unchanged)", ok, repr(text_of("b3_ck_notice")))
    check("history: the dialog names the child", "a7:walk-fork" in (text_of("b3_ck_forked") or ""),
          repr(text_of("b3_ck_forked")))
    grab("fork-done")
    click("b3_close") or key("Escape")
    wait(lambda: not is_shown("b3_ck_fork_name"), 4)
    open_drawer_if_phone()
    check("history: the sidebar lists the forked conversation",
          wait(lambda: session_row("walk-fork") is not None, 6), f"{session_titles()}")
    close_drawer_if_phone()
    send("/undo")
    if not check("history: /undo opens Undo workspace changes", wait(lambda: is_shown("b3_ck_snap_0_restore"), 6)):
        return
    grab("undo-list")
    click("b3_ck_snap_0_restore")
    check("history: Restore asks for confirmation first", wait(lambda: is_shown("b3_ck_confirm_btn"), 3))
    grab("undo-confirm")
    log_since()
    click("b3_ck_confirm_btn")
    check("history: Confirm restores the snapshot", wait(lambda: logged("snapshot/restore") or
                                                          "estored" in (text_of("b3_ck_notice") or ""), 8),
          repr(text_of("b3_ck_notice")))
    click("b3_close") or key("Escape")
    wait(lambda: not is_shown("b3_ck_scope"), 4)


def switcher_row(title_part):
    for w in snap():
        i = str(w.get("i", ""))
        if i.startswith("b3_switch_row_") and i.endswith("_title") and shown(w) and title_part in str(w.get("t", "")):
            return i[: -len("_title")]
    return None


def peer_walk():
    send("peer please")
    turn_idle(10)
    # The session switcher (/sessions) re-reads session/list: the staged
    # peer's own Session is listed there; a click opens it.
    send("/sessions")
    row = wait(lambda: switcher_row("review-diff") is not None, 8) and switcher_row("review-diff")
    if not check("peer: the staged peer's Session is listed (/sessions)", bool(row)):
        click("b3_close") or key("Escape")
        return
    click(f"{row}_tap")
    wait(lambda: not is_shown(f"{row}_tap"), 4)
    ok = wait(lambda: is_shown("peer_readonly"), 6)
    check("peer: a focused peer Session shows the read-only row", ok)
    check("peer: the editable composer is replaced", not is_shown("i0_composer_0"))
    check("peer: the row names the peer", "review-diff" in (text_of("peer_readonly_label") or ""),
          repr(text_of("peer_readonly_label")))
    grab("peer-readonly")
    open_session("Fix steer queue") or open_session("New chat")
    check("peer: back on the master the composer is editable", wait(lambda: is_shown("i0_composer_0"), 6))


def notifications_walk():
    path = os.environ.get("OCTOSCODE_NOTIFICATIONS_FILE", "")
    if path and os.path.exists(path):
        os.remove(path)
    close_drawer_if_phone()
    click("settings_open_hit")
    if not check("attention: Settings opens", wait(lambda: is_shown("settings_drawer"), 4)):
        return
    off = not inside("tg_on", "tg_notify")
    check("attention: Desktop notifications are off until the Settings action (no prompt)", off)
    if not is_shown("tg_notify"):
        # A25: a hidden test app is a bare binary, which macOS never lets
        # post a notice — the row reads "Unavailable" (no toggle, no prompt,
        # no consent). Launch with OCTOSCODE_NOTIFY_FAKE=granted to walk the
        # opt-in mechanics (tools/walk/a25_notifications.py walks every state).
        check("attention: no notification backend here — the row says Unavailable",
              text_of("notify_state") == "Unavailable")
        click("set_back" if MODE == "phone" else "settings_close")
        wait(lambda: not is_shown("settings_drawer"), 3)
        close_drawer_if_phone()
        return
    log_since()
    click("tg_notify")
    check("attention: the toggle opts in", wait(lambda: inside("tg_on", "tg_notify"), 3) and logged("notifications"))
    if path:
        saved = os.path.exists(path) and json.load(open(path)).get("enabled") is True
        check("attention: the opt-in is persisted", saved)
    grab("notifications-on")
    click("tg_notify")
    wait(lambda: not inside("tg_on", "tg_notify"), 3)
    if path:
        check("attention: turning it off persists too", json.load(open(path)).get("enabled") is False)
    click("set_back" if MODE == "phone" else "settings_close")
    wait(lambda: not is_shown("settings_drawer"), 3)
    close_drawer_if_phone()


def images_walk():
    shot = os.path.abspath(os.path.join("tmp", "a7-walk", "walk-shot.png"))
    png(shot)
    send("/images")
    if not check("media: /images opens the images dialog", wait(lambda: is_shown("b3_img_slot_0"), 6)):
        return
    r = rect("b3_img_slot_0")
    get(f"/drop?path={urllib.parse.quote(shot)}&x={r[0] + r[2] / 2:.0f}&y={r[1] + r[3] / 2:.0f}&wait=1")
    check("media: a dropped PNG fills slot 0 (selected, not uploaded)",
          wait(lambda: "Selected; not uploaded" in (text_of("b3_img_row_0") or ""), 4), repr(text_of("b3_img_row_0")))
    before = uploads_seen()
    n0 = len(before[0]) if before else 0
    # Upload twice in the same breath: one request per image (dedup).
    up = rect("b3_img_upload")
    if not check("media: Upload is offered", up is not None):
        return
    click_rect(up)
    click_rect(up)
    check("media: the entry is uploading", wait(lambda: "ploading" in (text_of("b3_img_row_0") or ""), 3),
          repr(text_of("b3_img_row_0")))
    # Cancel while in flight: the late receipt must not complete the entry.
    click("b3_img_cancel")
    check("media: Cancel uploads cancels locally",
          wait(lambda: "canceled locally" in (text_of("b3_img_notice") or ""), 3), repr(text_of("b3_img_notice")))
    time.sleep(4.5)
    after = uploads_seen()
    if after is not None:
        check("media: a double Upload sent ONE request", len(after[0]) - n0 == 1, f"{len(after[0]) - n0} POSTs")
        check("media: the cancelled transfer's receipt arrived late", len(after[1]) >= 1)
    check("media: the late receipt was ignored (not uploaded)",
          "Uploaded" not in (text_of("b3_img_row_0") or ""), repr(text_of("b3_img_row_0")))
    grab("images-cancelled")
    # Upload, then remove the entry while its transfer is in flight.
    click("b3_img_upload")
    wait(lambda: "ploading" in (text_of("b3_img_row_0") or ""), 3)
    grab("images-uploading")
    click("b3_img_slot_0_x")
    check("media: removing an in-flight entry frees the slot",
          wait(lambda: text_of("b3_img_row_0") is None, 3))
    time.sleep(4.5)
    check("media: its late receipt does not resurrect it", text_of("b3_img_row_0") is None)
    check("media: the picker is available again", is_shown("b3_img_choose"))
    click("b3_img_close_btn") or key("Escape")
    wait(lambda: not is_shown("b3_img_slot_0"), 4)


def seat_walk():
    held = wait(lambda: is_shown("hd_held"), 10)
    check("seat: the held banner names the other app", held and "octos-tui" in (text_of("hd_held_text") or ""),
          repr(text_of("hd_held_text")))
    ok, r = hit_ok("hd_take_over")
    check("layout: Take over >= 28 px", ok, f"{r}")
    grab("seat-held")
    log_since()
    send("please ship it")
    ok = wait(lambda: notice_with("Turn not sent", "Another app is using this session"), 8)
    check("seat: a held Session refuses the send with the human message", ok)
    check("seat: no protocol words reach the transcript", not texts_matching(lambda t: "ExternalMasterHeld" in t))
    check("seat: the draft is kept", wait(lambda: composer_text() == "please ship it", 4), repr(composer_text()))
    frames = served("<- turn/start")
    check("seat: no turn/start frame was written", frames == 0 if frames is not None else logged("not sent"),
          f"{frames} turn/start at the fixture")
    grab("seat-refused")
    log_since()
    click("hd_take_over")
    check("seat: Take over shows Resuming chat…",
          wait(lambda: (text_of("seat_status_label") or "").startswith("Resuming chat"), 3),
          repr(text_of("seat_status_label")))
    grab("seat-resuming")
    check("seat: acquire -> release(internal) -> one send",
          wait(lambda: logged("released (internal)"), 8) and wait(lambda: logged("Accepted"), 8))
    check("seat: the answer arrives",
          wait(lambda: bool(texts_matching(lambda t: "Done: please ship it" in t)), 8))
    check("seat: the banner is gone", wait(lambda: not is_shown("hd_held"), 4))
    check("seat: the status line is gone", not is_shown("seat_status"))
    grab("seat-resumed")


def draft_save():
    focus_composer()
    type_text("unsent draft survives a restart")
    time.sleep(1.2)
    check("draft: the text sits unsent in the composer", composer_text() == "unsent draft survives a restart")


def draft_check():
    ok = wait(lambda: composer_text() == "unsent draft survives a restart", 10)
    check("draft: the unsent text is back after the restart", ok, repr(composer_text()))
    ring = json.loads(get("/log?n=2000")).get("l", [])
    frames = served("<- turn/start")
    check("draft: it was restored, not sent",
          any("unsent draft restored" in l for l in ring) and (frames == 0 if frames is not None else True),
          f"{frames} turn/start at the fixture")
    grab("draft-restored")


def main():
    if MODE == "phone" and not is_shown("conversation_column"):
        # Open the app from the phone home (the OctosCode icon).
        get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    wait(lambda: is_shown("i0_composer_0") or is_shown("hd_held"), 20)
    log_since()
    if SCENARIO == "main":
        steps = {
            "markdown": markdown_walk, "shortcuts": shortcut_walk, "queue": queue_walk,
            "steer": steer_command_walk, "collision": collision_walk, "recovery": recovery_walk,
            "history": history_walk, "peer": peer_walk, "notifications": notifications_walk,
            "images": images_walk,
        }
        only = [s for s in os.environ.get("A7_WALK_STEPS", "").split(",") if s]
        for name, fn in steps.items():
            if not only or name in only:
                fn()
    elif SCENARIO == "seat":
        seat_walk()
    elif SCENARIO == "draft-save":
        draft_save()
    elif SCENARIO == "draft-check":
        draft_check()
    failed = [n for n, ok, _ in RESULTS if not ok]
    print(f"== WALK {MODE} {SCENARIO}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
