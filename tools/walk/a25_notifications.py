#!/usr/bin/env python3
"""A25 — desktop notifications: the Settings row's states and the routed click
(parity rows 276, 322, 323).

    OCTOSCODE_APP_BIN=<host bin> python3 tools/walk/a25_notifications.py [desktop|phone|both] [outdir]

Every phase launches the HOST app hidden (harness/headless.sh, isolated state —
brief §8) on A10_PORT (default 8512) with the replay server on A10_REPLAY_PORT
(default 8514; `activity` scenario: five Sessions, `--adopt-turn-ids` so a sent
prompt plays as the app's own turn), drives it by CLICKS at /snap rects, reads
the app's own log, and stops both.

The OS layer. A hidden test app is a bare binary, not an `.app` bundle, and
macOS never lets such a process post a notice: the real platform layer
answers "unavailable" (phase `real`). Every other phase runs the app's fake OS
(`OCTOSCODE_NOTIFY_FAKE=<mode>`, src/attention.rs `FakeOs`), which answers
through the SAME makepad actions the platform posts and logs each post/close.
A real OS click cannot reach a hidden window, so the routed click is injected
through a TEST-ONLY hook, the instrument's
`/event?data=octoscode.attention.click:<notice id>`, which posts the platform's
own `NotificationClicked` action (the arm a real click takes); window focus
likewise uses `octoscode.attention.focus:1|0`, which calls the
WindowGotFocus/WindowLostFocus handler (a hidden window never gains focus).
The real OS round trip is tools/walk/a25_live_macos.py.

Captures (PNG <= 1400 px + scrubbed /snap) and walk.log under <outdir>/<mode>/.
"""
from __future__ import annotations

import json
import os
import pathlib
import sys
import time
import urllib.parse

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import a10_lib  # noqa: E402
from a10_lib import inside, overlap  # noqa: E402
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
WALK = {
    "name": "a25_notifications",
    "title": "desktop notifications: the Settings row's states, a notice when a turn finishes while the window "
             "is not focused, the click opens its Session, focus acknowledges",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 1500,
    "rows": {
        4: {"checks": ["granted: off until the Settings action", "granted: the toggle asks the OS once and turns on",
                       "granted: off again without asking the OS", "click: a focused window never notifies"],
            "partial": "the browser tab title is not a native surface (row 319)"},
        5: {"checks": ["click: the finished turn posted ONE notice", "click: later syncs cannot notify twice",
                       "click: window focus acknowledges", "click: returning (focus) withdraws the notice"],
            "partial": "the title count is not a native surface; a hidden window never gains focus, so focus is "
                       "the test-only hook octoscode.attention.focus (the WindowGotFocus handler)"},
        6: {"checks": ["denied: the toggle stays off and the message is an alert"],
            "partial": "Disconnect clearing attention is unit-tested "
                       "(attention::tests::disconnect_and_identity_change_reset_and_withdraw), not walked"},
    },
}

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
OUT = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "docs" / "ux" / "a25"
WHICH = sys.argv[1] if len(sys.argv) > 1 else "both"

DEFAULT = "Notify when a turn needs you or finishes while OctosCode is in the background"
ON = "Desktop notifications are on."
UNAVAILABLE = "Desktop notifications are unavailable here."
BLOCKED = "Notifications are blocked. Allow OctosCode in System Settings › Notifications."
FAILED = "Could not enable desktop notifications. Try again when permissions allow."
DISMISSED = "Permission was not granted. You can enable notifications later."
SESSION_A, TITLE_A = "dsflash:main", "Fix steer queue drop on reconnect"
TITLE_B = "Add session fork"
NOTICE_A = "octoscode-attention:" + SESSION_A
FINISHED = "A background response finished. Return to OctosCode to review it."

# Every row-state capture with its numbers: (mode, state) -> checks.
UX: dict[tuple[str, str], dict] = {}


# ------------------------------------------------------------------- helpers
class Log:
    """Every app log line since the walk began (a10_lib.log_since consumes)."""

    def __init__(self, w: a10_lib.Walk):
        self.w = w
        self.lines: list[str] = []

    def pull(self) -> list[str]:
        new = self.w.log_since()
        self.lines.extend(new)
        return new

    def mark(self) -> int:
        self.pull()
        return len(self.lines)

    def since(self, mark: int, needle: str) -> list[str]:
        self.pull()
        return [l for l in self.lines[mark:] if needle in l]

    def wait(self, mark: int, needle: str, secs: float = 8.0) -> bool:
        return self.w.wait(lambda: bool(self.since(mark, needle)), secs, 0.3)


def hook(w: a10_lib.Walk, data: str) -> None:
    w.note(f"HOOK /event?data={data}")
    w.get("/event?" + urllib.parse.urlencode({"data": data}), tolerant=True)
    time.sleep(0.4)


def label(sn, text: str):
    return next((x for x in sn if x.get("ty") == "Label" and (x.get("t") or "") == text and a10_lib.Walk.shown(x)), None)


def open_settings(w: a10_lib.Walk) -> bool:
    if w.mode == "phone" and w.visible("drawer_scrim"):
        w.click("drawer_close")
        time.sleep(0.8)
    w.click("settings_open_hit")
    return w.wait(lambda: bool(w.visible("notify_row")), 8)


def close_settings(w: a10_lib.Walk) -> None:
    w.click("set_back" if w.mode == "phone" else "settings_close")
    w.wait(lambda: not w.visible("settings_drawer"), 4)


def row(w: a10_lib.Walk) -> dict:
    sn = w.snap()
    return {
        "toggle": bool(w.visible("tg_notify", sn)),
        "on": inside_layer(w, sn, "tg_on"),
        "state": w.text("notify_state", sn),
        "help": w.text("notify_help", sn),
        "alert": w.text("notify_alert", sn),
    }


def inside_layer(w, sn, layer: str) -> bool:
    tg = w.rect("tg_notify", sn=sn)
    return bool(tg) and any(inside(x["r"], tg) for x in w.visible(layer, sn))


def red_pixels(w: a10_lib.Walk, rect) -> int:
    """Danger-ink pixels inside `rect` (window coordinates) of a raw grab."""
    try:
        import io
        from PIL import Image

        sn = w.snap()
        win = next((x["r"] for x in sn if x.get("ty") == "Window" and w.shown(x)), None)
        data = w.get_bytes("/g?raw=1")
        img = Image.open(io.BytesIO(data)).convert("RGB")
        k = img.width / win[2]
        x0, y0 = int((rect[0] - win[0]) * k), int((rect[1] - win[1]) * k)
        x1, y1 = int((rect[0] + rect[2] - win[0]) * k), int((rect[1] + rect[3] - win[1]) * k)
        n = 0
        for y in range(y0, y1):
            for x in range(x0, x1):
                r, g, b = img.getpixel((x, y))
                if r > 170 and g < 110 and b < 110:
                    n += 1
        return n
    except Exception as e:  # noqa: BLE001
        w.note(f"red pixel probe failed: {e}")
        return -1


def ux_checks(w: a10_lib.Walk, state: str) -> dict:
    """The row's numbers (brief §2): every text inside the row, the control
    flush right, the message clear of the right gutter, no overlaps, hit
    targets >= 28 px, the alert in the danger ink."""
    sn = w.snap()
    r = w.rect("notify_row", sn=sn)
    title = label(sn, "Desktop notifications")
    theme = label(sn, "Theme")
    control = w.rect("tg_notify", sn=sn) or w.rect("notify_state", sn=sn)
    message = w.rect("notify_alert", sn=sn) or w.rect("notify_help", sn=sn)
    c: dict = {}
    c["title inside row"] = bool(r and title and inside(title["r"], r))
    c["control inside row"] = bool(r and control and inside(control, r))
    c["control flush right"] = bool(r and control and abs((control[0] + control[2]) - (r[0] + r[2])) <= 1.5)
    c["message inside row"] = bool(r and message and inside(message, r))
    c["message clear of the control column"] = bool(r and message and message[0] + message[2] <= r[0] + r[2] - 64 + 1.5)
    c["title/control no overlap"] = bool(title and control and not overlap(title["r"], control))
    c["message/control no overlap"] = bool(message and control and not overlap(message, control))
    c["left edge = Theme row"] = bool(title and theme and abs(title["r"][0] - theme["r"][0]) <= 1.0)
    hit = w.rect("tg_hit", sn=sn)
    c["hit >= 28 px"] = (hit[2] >= 28 and hit[3] >= 28) if hit else state in ("pending", "unavailable")
    if w.visible("notify_alert", sn):
        c["alert in danger ink"] = red_pixels(w, w.rect("notify_alert", sn=sn)) > 40
    c["_rects"] = {"row": r, "title": title["r"] if title else None, "control": control, "message": message}
    return c


def capture(w: a10_lib.Walk, name: str, state: str) -> None:
    c = ux_checks(w, state)
    w.shot(name)
    snp = w.out / f"{name}.snap.json"
    snp.write_text(a10_lib.scrub(snp.read_text()))
    passed = [k for k, v in c.items() if not k.startswith("_") and v]
    failed = [k for k, v in c.items() if not k.startswith("_") and not v]
    w.check(f"ux {name}: {len(passed)}/{len(passed) + len(failed)} numeric checks", not failed, f"failed={failed} rects={c['_rects']}")
    UX[(w.mode, state)] = c


def get_bytes(self, path: str) -> bytes:
    import urllib.request

    with urllib.request.urlopen(self.base + path, timeout=30) as r:
        return r.read()


a10_lib.Walk.get_bytes = get_bytes  # a raw grab for the pixel probe


PREF = {"path": ""}


def run(mode: str, phase: str, fn, fake: str | None) -> int:
    # The opt-in file the app writes (isolated, never the operator's; tmp/ is
    # not committed). The walk reads it back.
    pref = ROOT / "tmp" / "a25" / f"{mode}-{phase}-notifications.json"
    pref.parent.mkdir(parents=True, exist_ok=True)
    pref.unlink(missing_ok=True)
    PREF["path"] = str(pref)
    env = {"OCTOSCODE_NOTIFY_FAKE": fake or "", "OCTOSCODE_NOTIFICATIONS_FILE": str(pref)}
    return a10_lib.run_session(
        fn, mode=mode, outdir=str(OUT / mode / phase), scenario="activity",
        port=int(os.environ.get("A10_PORT", "8512")),
        replay_port=int(os.environ.get("A10_REPLAY_PORT", "8514")),
        env=env, replay_args=["--adopt-turn-ids"],
    )


# -------------------------------------------------------------------- phases
def phase_real(w: a10_lib.Walk) -> None:
    """The hidden app is a bare binary: the platform layer says unavailable."""
    log = Log(w)
    w.check("real: the platform layer reported this process unavailable",
            bool(log.since(0, "attention: os Authorization { status: Unavailable, requested: false")),
            "macOS UNUserNotificationCenter needs an .app bundle")
    if not w.check("real: Settings > General shows the Desktop notifications row", open_settings(w)):
        return
    w.wait(lambda: row(w)["state"] == "Unavailable", 4)
    r = row(w)
    w.check("real: the row reads Unavailable, no toggle to press (the web's disabled button)",
            r["state"] == "Unavailable" and not r["toggle"], json.dumps(r))
    w.check("real: the message says so", r["help"] == UNAVAILABLE, r["help"])
    m = log.mark()
    rect = w.rect("notify_state")
    if rect:
        w.click_xy(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2)
    time.sleep(0.8)
    w.check("real: clicking there asks nothing", not log.since(m, "notifications_toggle.toggle"))
    capture(w, "row-unavailable", "unavailable")


def phase_granted(w: a10_lib.Walk) -> None:
    log = Log(w)
    path = PREF["path"]
    if not w.check("granted: Settings > General shows the row", open_settings(w)):
        return
    w.wait(lambda: row(w)["toggle"], 4)
    r = row(w)
    w.check("granted: off until the Settings action (no prompt yet)",
            r["toggle"] and not r["on"] and r["help"] == DEFAULT and not log.since(0, "requested=true"), json.dumps(r))
    capture(w, "row-off", "off")
    m = log.mark()
    w.click("tg_notify")
    w.wait(lambda: row(w)["on"], 5)
    r = row(w)
    w.check("granted: the toggle asks the OS once and turns on (aria-pressed)",
            r["on"] and r["help"] == ON and len(log.since(m, "authorization granted requested=true")) == 1, json.dumps(r))
    if path:
        w.check("granted: the opt-in is persisted",
                os.path.exists(path) and json.load(open(path)).get("enabled") is True)
    capture(w, "row-on", "on")
    m = log.mark()
    w.click("tg_notify")
    w.wait(lambda: not row(w)["on"], 5)
    r = row(w)
    w.check("granted: off again without asking the OS",
            not r["on"] and r["help"] == DEFAULT and not log.since(m, "requested=true"), json.dumps(r))
    if path:
        w.check("granted: turning it off persists too", json.load(open(path)).get("enabled") is False)


def phase_pending(w: a10_lib.Walk) -> None:
    log = Log(w)
    if not w.check("pending: Settings > General shows the row", open_settings(w)):
        return
    w.wait(lambda: row(w)["toggle"], 4)
    w.click("tg_notify")
    w.wait(lambda: row(w)["state"] == "Enabling…", 5)
    r = row(w)
    w.check("pending: 'Enabling…' while the OS prompt waits, no toggle to press twice",
            r["state"] == "Enabling…" and not r["toggle"], json.dumps(r))
    m = log.mark()
    rect = w.rect("notify_state")
    if rect:
        w.click_xy(rect[0] + rect[2] / 2, rect[1] + rect[3] / 2)
    time.sleep(0.8)
    w.check("pending: a second click asks nothing", not log.since(m, "request left pending"))
    capture(w, "row-pending", "pending")


def phase_alert(state: str, message: str):
    def walk(w: a10_lib.Walk) -> None:
        log = Log(w)
        if not w.check(f"{state}: Settings > General shows the row", open_settings(w)):
            return
        w.wait(lambda: row(w)["toggle"], 4)
        w.click("tg_notify")
        w.wait(lambda: row(w)["alert"] != "", 5)
        r = row(w)
        w.check(f"{state}: the toggle stays off and the message is an alert",
                r["toggle"] and not r["on"] and r["alert"] == message and r["help"] == "", json.dumps(r))
        w.check(f"{state}: one request", len(log.since(0, "requested=true")) == 1)
        capture(w, f"row-{state}", state)
    return walk


def phase_click(w: a10_lib.Walk) -> None:
    """use-attention.ts / attention.spec.ts on the real app: a turn finishing
    while the window is not focused notifies once; a click on the notice opens
    its Session; focus acknowledges and withdraws; a focused window never
    notifies for the Session it shows."""
    log = Log(w)
    w.check("click: the app starts unfocused (a hidden window never gains focus)",
            bool(log.since(0, "attention: start (platform api true, focused false)")))
    if not open_settings(w):
        w.check("click: Settings opens", False)
        return
    w.click("tg_notify")
    w.wait(lambda: row(w)["on"], 5)
    w.check("click: notifications opted in", row(w)["on"])
    close_settings(w)

    def send(text: str) -> None:
        if w.mode == "phone" and w.visible("drawer_scrim"):
            w.click("drawer_close")
            time.sleep(0.6)
        w.click("i0_composer_0")
        w.type_text(text)
        w.key("Return")

    m = log.mark()
    send("Summarize the task list")
    posted = log.wait(m, "attention(fake os): posted", 30)
    lines = log.since(m, "attention(fake os): posted")
    w.check("click: the finished turn posted ONE notice for its Session (title = the Session, the web's body)",
            posted and len(lines) == 1 and f"posted {NOTICE_A} | {TITLE_A} | {FINISHED}" in lines[0],
            lines[0][-160:] if lines else "none")
    w.check("click: posting never focused the app",
            not log.since(m, "window focused") and not log.since(m, "notice click"))
    time.sleep(2.5)
    w.check("click: later syncs cannot notify twice", len(log.since(m, "attention(fake os): posted")) == 1)
    w.shot("notice-posted")

    # Switch to another Session (still unfocused): the notice stays.
    if w.mode == "phone":
        w.click("sidebar_toggle_hit")
        w.wait(lambda: bool(w.visible("drawer_scrim")), 4)
    sn = w.snap()
    row_b = next((x for x in sn if x.get("i") == "sb_r_title" and x.get("t") == TITLE_B and w.shown(x)), None)
    m = log.mark()
    if row_b:
        x, y, ww, hh = row_b["r"]
        w.note(f"CLICK the '{TITLE_B}' row at ({x + ww / 2:.0f},{y + hh / 2:.0f})")
        w.click_xy(x + ww / 2, y + hh / 2)
    switched = log.wait(m, "sidebar action: session.open", 6)
    w.wait(lambda: header_title(w) == TITLE_B, 8)
    w.check("click: switched to another Session by its sidebar row", switched and header_title(w) == TITLE_B,
            f"header={header_title(w)!r}")
    w.check("click: the notice was not withdrawn by the switch", not log.since(m, f"closed {NOTICE_A}"))

    # The click on the notice (test-only hook = the platform's own action).
    m = log.mark()
    hook(w, "octoscode.attention.click:" + NOTICE_A)
    routed = log.wait(m, f"notice click -> open {SESSION_A}", 6)
    w.wait(lambda: header_title(w) == TITLE_A, 8)
    w.check("click: the click opened the notice's Session", routed and header_title(w) == TITLE_A,
            f"header={header_title(w)!r}")
    w.check("click: the clicked notice was withdrawn", bool(log.since(m, f"closed {NOTICE_A}")))
    if w.mode == "phone" and w.visible("drawer_scrim"):
        w.click("drawer_close")
        time.sleep(0.6)
    w.shot("click-routed")

    # Focus acknowledges.
    m = log.mark()
    hook(w, "octoscode.attention.focus:1")
    w.check("click: window focus acknowledges", log.wait(m, "window focused — acknowledged", 4))

    # Focused: a finished turn of the shown Session never notifies.
    m = log.mark()
    played = w.replay_saw("turn/start", 1)
    send("And now the follow-up")
    w.wait(lambda: w.replay_saw("turn/start", 1) > played, 10)
    time.sleep(6.0)
    w.check("click: a focused window never notifies for the Session it shows",
            w.replay_saw("turn/start", 1) > played
            and not log.since(m, "attention(fake os): posted") and not log.since(m, "needs attention"),
            f"turn/start sent {w.replay_saw('turn/start', 1)}x (the turn played)")

    # Unfocused again: the next finished turn notifies again.
    hook(w, "octoscode.attention.focus:0")
    m = log.mark()
    send("One more")
    w.check("click: unfocused again, the next finished turn notifies", log.wait(m, "attention(fake os): posted", 30))

    # attention.spec.ts:155 — returning to the window (focus) withdraws it.
    m = log.mark()
    hook(w, "octoscode.attention.focus:1")
    w.check("click: returning (focus) withdraws the notice",
            log.wait(m, f"closed {NOTICE_A}", 4) and log.wait(m, "window focused — acknowledged", 2))


def header_title(w: a10_lib.Walk) -> str:
    sn = w.snap()
    t = w.text("hd_title", sn)
    if t:
        return t
    # The phone header names the Session in its title row.
    for x in sn:
        if x.get("ty") == "Label" and w.shown(x) and x["r"][1] < 140 and x.get("t") in (TITLE_A, TITLE_B):
            return x["t"]
    return ""


PHASES = {
    "real": (lambda: phase_real, None),
    "granted": (lambda: phase_granted, "granted"),
    "pending": (lambda: phase_pending, "pending"),
    "denied": (lambda: phase_alert("denied", BLOCKED), "denied"),
    "error": (lambda: phase_alert("error", FAILED), "error"),
    "dismissed": (lambda: phase_alert("dismissed", DISMISSED), "default"),
    "click": (lambda: phase_click, "granted"),
}


def main() -> int:
    modes = ["desktop", "phone"] if WHICH == "both" else [WHICH]
    only = os.environ.get("A25_PHASES", "").split(",") if os.environ.get("A25_PHASES") else list(PHASES)
    rc = 0
    for mode in modes:
        for phase in only:
            fn, fake = PHASES[phase]
            rc |= run(mode, phase, fn(), fake)
    summary = {f"{m}/{s}": {k: v for k, v in c.items()} for (m, s), c in UX.items()}
    (OUT / "ux-checks.json").write_text(json.dumps(summary, indent=1, default=str))
    print("ALL PASS" if rc == 0 else "SOME FAILED", flush=True)
    return rc


if __name__ == "__main__":
    sys.exit(main())
