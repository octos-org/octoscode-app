#!/usr/bin/env python3
"""A26 — shared steps of the A26 click walks (a26_palettes, a26_footer,
a26_toasts): the footer and drawer by CLICK, Settings > Preferences, and the
pixel proof that the app DRAWS a look (the /g capture sampled where the
window's own background shows, compared with the look's colour).

Never imported by the aggregator (tools/walk/native.py reads the walks'
WALK literals with ast); the walks import it.
"""
from __future__ import annotations

import io
import json
import pathlib
import sys
import time
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import a10_lib  # noqa: E402
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

# The window background each look draws (screens/theme.rs: the light /
# dark shell roles, each named palette's --display-surface).
SURFACE = {
    "light": "#ffffff",
    "dark": "#1c1f22",
    "codex": "#0f1218",
    "claude": "#261f1a",
    "slate": "#141923",
    "solarized": "#002b36",
}
PALETTES = ["system", "light", "dark", "codex", "claude", "slate", "solarized"]
LABELS = {"system": "System", "light": "Light", "dark": "Dark", "codex": "Codex", "claude": "Claude", "slate": "Slate", "solarized": "Solarized"}


def walk_from_argv(default_mode: str = "desktop") -> a10_lib.Walk:
    """The aggregator's argv: `{port} {mode} {fixture_log} {out}`."""
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8518
    mode = sys.argv[2] if len(sys.argv) > 2 else default_mode
    fixture_log = sys.argv[3] if len(sys.argv) > 3 else ""
    out = sys.argv[4] if len(sys.argv) > 4 else "tmp/a26-walk"
    w = a10_lib.Walk(port, out, mode)
    if fixture_log:
        w.replay_log = pathlib.Path(fixture_log)
    return w


def hex_rgb(h: str) -> tuple:
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def near(a: tuple, b: tuple, tol: int = 6) -> bool:
    return all(abs(x - y) <= tol for x, y in zip(a, b))


def grab(w: a10_lib.Walk):
    """The raw /g frame and the window's logical rect (for the scale)."""
    from PIL import Image  # noqa: PLC0415

    sn = w.snap()
    win = next((x["r"] for x in sn if x.get("ty") == "Window" and w.shown(x)), None)
    for _ in range(6):
        try:
            data = urllib.request.urlopen(w.base + "/g?raw=1", timeout=30).read()
            if data[:4] == b"\x89PNG":
                img = Image.open(io.BytesIO(data)).convert("RGB")
                return img, win, sn
        except Exception:  # noqa: BLE001 — a missed frame: retry
            pass
        time.sleep(0.6)
    return None, win, sn


def pixel(w: a10_lib.Walk, x: float, y: float):
    """The drawn colour at logical (x, y), as (r, g, b)."""
    img, win, _ = grab(w)
    if img is None or not win:
        return None
    # The capture's origin is the window's (a10_lib.Walk.shot crops with the
    # same scale).
    k = img.width / win[2]
    return img.getpixel((min(img.width - 1, int(x * k)), min(img.height - 1, int(y * k))))


def sidebar_spot(w: a10_lib.Walk, sn=None):
    """An empty spot of the sidebar column (above + Add workspace): the
    window's own background (`color_bg_app`)."""
    col = w.rect("threads_column", sn=sn)
    add = w.rect("sb_add_hit", sn=sn)
    if not col or not add:
        return None
    return (col[0] + col[2] * 0.6, add[1] - 24)


def conversation_spot(w: a10_lib.Walk, sn=None):
    """The conversation's left gutter just under its header: the window's
    own background (the phone's drawer is closed, so the sidebar is not on
    screen)."""
    col = w.rect("conversation_column", sn=sn)
    head = w.rect("oc_header", sn=sn)
    if not col or not head:
        return None
    return (col[0] + 4, head[1] + head[3] + 6)


def draws_look(w: a10_lib.Walk, look: str):
    """(ok, detail): the window's own background — the sidebar's empty area
    on a desktop, the conversation's gutter on a phone — is the look's
    surface."""
    sn = w.snap()
    spot = sidebar_spot(w, sn) or conversation_spot(w, sn)
    if not spot:
        return False, "neither the sidebar nor the conversation is on screen"
    got = pixel(w, *spot)
    want = hex_rgb(SURFACE[look])
    return (got is not None and near(got, want)), f"pixel {got} at {tuple(round(v) for v in spot)} vs {look} {want}"


def open_drawer(w: a10_lib.Walk) -> bool:
    """Phone: the header's menu opens the sidebar drawer (no-op on desktop)."""
    if w.mode != "phone" or w.visible("sb_settings_hit"):
        return True
    w.click("sidebar_toggle_hit")
    return w.wait_shown("sb_settings_hit", 6)


def open_settings_from_footer(w: a10_lib.Walk) -> bool:
    """The sidebar footer's Settings entry (the web's placement)."""
    open_drawer(w)
    if not w.click("sb_settings_hit"):
        return False
    return w.wait(lambda: bool(w.visible("set_title")), 6)


def open_preferences(w: a10_lib.Walk) -> bool:
    nav = "set_rail_preferences" if w.mode == "phone" else "set_nav_preferences"
    if not w.click(nav):
        return False
    return w.wait(lambda: w.text("set_title") == "Preferences", 6)


def close_settings(w: a10_lib.Walk) -> bool:
    w.click("set_back" if w.mode == "phone" else "settings_close")
    return w.wait(lambda: not w.visible("set_title"), 6)


def logged(w: a10_lib.Walk, needle: str, secs: float = 6.0) -> bool:
    return w.logged(needle, secs)


def inside(inner, outer, tol=1.0) -> bool:
    return a10_lib.inside(inner, outer, tol)


def overlap(a, b) -> bool:
    return a10_lib.overlap(a, b)


def dump(w: a10_lib.Walk, name: str, data) -> None:
    (w.out / f"{name}.json").write_text(json.dumps(data, indent=1))
