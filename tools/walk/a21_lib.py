#!/usr/bin/env python3
"""A21 — shared helpers for the connection walks (a21_bootstrap_walk.py,
a21_drafts_walk.py): the hidden app and its fixtures are launched with
ISOLATED state (walk_env, brief §8 — never ~/.octoscode), every step is a
CLICK at a laid-out rect (or typing into the focused field), and every
capture is saved with its /snap scrubbed (snapsafe: no field value of a
secret-like widget; walk_env.scrub: no machine path).

No token ever reaches a log line, a capture's JSON or a check detail: the
token field is only ever measured (length), never printed.
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import time
import urllib.parse

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge's per-launch token on every request)
from snapsafe import scrub as scrub_secrets  # noqa: E402
from walk_env import ROOT, App, Fixture, app_env, scrub  # noqa: E402

EXAMPLES = ROOT / "target" / "debug" / "examples"


def origin(port: int) -> str:
    return f"http://127.0.0.1:{port}"


class Walk:
    """One mode's walk: its app (relaunched on the same isolated state), the
    checks it printed, its captures."""

    def __init__(self, binary: str, mode: str, port: int, out: pathlib.Path, env_extra: dict):
        self.binary, self.mode, self.port = binary, mode, port
        self.out = out
        self.out.mkdir(parents=True, exist_ok=True)
        self.state = self.out / "state"
        self.env_extra = env_extra
        self.app: App | None = None
        self.results: list[tuple[str, bool, str]] = []
        self.lines: list[str] = []
        self.launches = 0

    # ------------------------------------------------------------ output
    def say(self, line: str):
        line = scrub(line)
        print(line, flush=True)
        self.lines.append(line)

    def check(self, name: str, ok, detail: str = "") -> bool:
        ok = bool(ok)
        self.results.append((name, ok, detail))
        self.say(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
        return ok

    def finish(self, title: str) -> int:
        failed = [n for n, ok, _ in self.results if not ok]
        self.say(f"== WALK {title} {self.mode}: {len(self.results) - len(failed)}/{len(self.results)} passed")
        (self.out / "walk.log").write_text("\n".join(self.lines) + "\n")
        return 1 if failed else 0

    # ------------------------------------------------------------ the app
    def launch(self, ready=("connect_btn", "i0_composer_0", "b1_connect_pair", "sidebar_toggle_hit")) -> bool:
        env = app_env(self.mode, self.state, self.env_extra, hs=self.out / "hs")
        # The product path: no harness identity (brief: the harness start is
        # test-only and is NOT what these walks prove).
        for k in ("OCTOS_PROFILE_ID", "OCTOS_BEARER", "OCTOS_PAIRING_LINK", "OCTOS_CREATE_PROFILE"):
            env.pop(k, None)
        env.update(self.env_extra)
        self.app = App(self.port, env)
        self.launches += 1
        ok = self.app.start(self.binary, ready_ids=ready)
        if self.mode == "phone":
            # The phone shell opens OctosCode by its launch action; wait for it.
            self.app.wait(lambda: self.shown("connect_card") or self.shown("i0_composer_0"), 40)
        time.sleep(2.0)
        return ok

    def stop(self):
        if self.app:
            self.app.stop()
            self.app = None

    # ------------------------------------------------------------ reading
    def snap(self):
        return self.app.snap()

    def shown(self, wid, s=None) -> bool:
        return self.app.find(wid, s) is not None

    def rect(self, wid, s=None):
        w = self.app.find(wid, s)
        return w["r"] if w else None

    def text(self, wid, s=None) -> str:
        return self.app.text(wid, s)

    def val(self, wid, s=None) -> str:
        w = self.app.find(wid, s)
        return (w or {}).get("val") or ""

    def wait(self, pred, timeout=12.0):
        return self.app.wait(pred, timeout=timeout)

    def logs(self) -> list:
        return self.app.logs()

    def all_logs(self) -> list:
        return self.app.all_logs()

    # ------------------------------------------------------------ state
    def card(self) -> bool:
        return self.shown("connect_card") and not self.shown("i0_composer_0")

    def live(self) -> bool:
        return self.shown("i0_composer_0") and not self.shown("connect_card")

    def composer(self) -> str:
        w = self.app.find("i0_composer_0")
        if not w:
            return ""
        v = w.get("val")
        if v is None:
            v = w.get("t") or ""
        return "" if v == "Ask Octos anything" else v

    # ------------------------------------------------------------ input
    def click(self, wid, nth=0) -> bool:
        return self.app.click(wid, nth)

    def type(self, text: str):
        self.app.get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
        time.sleep(0.3)

    def key(self, code: str, n: int = 1):
        for _ in range(n):
            self.app.get(f"/k?c={code}&wait=1")

    def clear_focused(self, n: int):
        """End, then Backspace past every character (the instrument's Cmd+A
        does not reach a TextInput's select-all)."""
        self.key("end")
        self.key("backspace", n + 1)
        time.sleep(0.2)

    def set_field(self, wid: str, text: str) -> bool:
        if not self.click(wid):
            return False
        time.sleep(0.2)
        self.clear_focused(max(len(self.val(wid)), 48))
        self.type(text)
        return True

    def connect(self, server: str | None, token: str | None) -> bool:
        """Fill the Connect card (by clicks + typing) and press Connect."""
        if server is not None and self.val("connect_server") != server:
            self.set_field("connect_server", server)
        if token is not None:
            self.set_field("connect_token", token)
        self.logs()
        return self.click("connect_btn")

    def open_settings(self) -> bool:
        if not self.click("settings_open_hit"):
            return False
        return bool(self.wait(lambda: self.shown("settings_drawer"), 8))

    def section(self, name: str) -> bool:
        sections = ["general", "permissions", "model", "sandbox", "connection", "preferences", "about"]
        cell = "rl_hit" if self.mode == "phone" else "nv_hit"
        self.click(cell, sections.index(name))
        return bool(self.wait(lambda: self.shown(f"sec_{name}"), 6))

    def leave(self, kind: str) -> bool:
        """Settings > Connection > Disconnect | Forget server (confirming the
        A9 dialog when it asks) -> the Connect card."""
        if not self.open_settings() or not self.section("connection"):
            return False
        self.click("settings_disconnect" if kind == "disconnect" else "settings_forget")
        if self.wait(lambda: self.shown("a9_lv_dialog"), 3):
            self.click("a9_lv_confirm")
        return bool(self.wait(lambda: self.card(), 15))

    def sidebar_open(self):
        if self.mode == "phone" and not self.shown("sb_new_chat_hit"):
            self.click("sidebar_toggle_hit")
            self.wait(lambda: self.shown("sb_new_chat_hit"), 4)

    def open_session(self, title: str) -> bool:
        """Click the sidebar row whose title is `title` (drawn whole or cut
        with '…')."""
        def row():
            for w in self.snap():
                t = w.get("t") or ""
                if w.get("i") == "sb_r_title" and App.shown(w) and (
                        t == title or (t.endswith("…") and title.startswith(t[:-1].rstrip()))):
                    return w
            return None

        self.sidebar_open()
        r = self.wait(row, 15)
        if not r:
            return False
        x, y, w, h = r["r"]
        self.app.click_xy(x + w / 2, y + h / 2)
        time.sleep(1.2)
        return True

    def type_in_composer(self, text: str):
        r = self.rect("i0_composer_0")
        if r:
            self.app.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        time.sleep(0.2)
        n = len(self.composer())
        if n:
            self.clear_focused(n)
        self.type(text)

    # ------------------------------------------------------------ evidence
    def shot(self, name: str):
        png = self.app.png()
        if png is None:
            self.say(f"NOTE capture {name} unavailable")
            return
        path = self.out / f"{name}.png"
        path.write_bytes(png)
        try:
            from PIL import Image  # noqa: PLC0415
            im = Image.open(path)
            if im.width > 1400:
                im = im.resize((1400, round(im.height * 1400 / im.width)))
                im.save(path)
        except ImportError:
            subprocess.run(["sips", "-Z", "1400", str(path)], capture_output=True)
        tree = scrub_secrets(self.snap())
        (self.out / f"{name}.snap.json").write_text(scrub(json.dumps(tree)))
        self.say(f"SHOT {name}")


# ---------------------------------------------------------------- UX numbers
def inside(inner, outer, slack=1.0) -> bool:
    return bool(inner and outer) and (
        inner[0] >= outer[0] - slack and inner[1] >= outer[1] - slack
        and inner[0] + inner[2] <= outer[0] + outer[2] + slack
        and inner[1] + inner[3] <= outer[1] + outer[3] + slack)


def overlaps(a, b) -> bool:
    return a[0] < b[0] + b[2] - 0.5 and b[0] < a[0] + a[2] - 0.5 and a[1] < b[1] + b[3] - 0.5 and b[1] < a[1] + a[3] - 0.5


def card_checks(w: Walk, tag: str) -> bool:
    """The Connect card by numbers: inside the module, centred in its pane,
    every label/field/control inside the card, controls >= 28 px high, the
    stacked rows never overlap, no label wider than the card."""
    s = w.snap()
    card = w.rect("connect_card", s)
    mod = next((x["r"] for x in s if x.get("i") == "connect_center" and App.shown(x)), None)
    ids = ["connect_title", "connect_server", "connect_token", "connect_note", "connect_btn", "connect_btn_label",
           "connect_pair_label", "connect_solo_label"]
    rects = {i: w.rect(i, s) for i in ids}
    out = [i for i, r in rects.items() if r and not inside(r, card)]
    # The tap targets (the fields' 42 px boxes are anonymous; their inputs'
    # rects are the text line).
    controls = {i: w.rect(i, s) for i in ("connect_btn", "b1_connect_pair", "connect_solo", "connect_eye")}
    controls = {i: r for i, r in controls.items() if r}
    small = [i for i, r in controls.items() if r[3] < 28]
    stack = [rects[i] for i in ("connect_title", "connect_server", "connect_token", "connect_note", "connect_btn") if rects[i]]
    over = [(i, j) for i in range(len(stack)) for j in range(i + 1, len(stack)) if overlaps(stack[i], stack[j])]
    centred = None
    if card and mod:
        # The card is centred in the pane right of the first-run sidebar
        # (lib.rs mount_connect_card: left = FIRST_RUN_SIDEBAR_W 261 on the
        # desktop, 0 on a phone; connect_center pads 16 more each side).
        lp = (261.0 if w.mode == "desktop" else 0.0) + 16.0
        pane_cx = mod[0] + lp + (mod[2] - lp - 16.0) / 2
        centred = abs((card[0] + card[2] / 2) - pane_cx)
    ok = bool(card) and not out and not small and not over and (centred is None or centred <= 2.0)
    w.check(f"UX {tag}: the Connect card by numbers",
            ok, f"card={card} pane={mod} centre_dx={None if centred is None else round(centred, 1)} "
                f"outside={out} under28={small} overlaps={len(over)}")
    return ok


def composer_checks(w: Walk, tag: str) -> bool:
    """The live shell by numbers: the composer inside the conversation
    column, >= 28 px high, the column using the window sensibly."""
    s = w.snap()
    comp = w.rect("i0_composer_0", s)
    row = w.rect("composer_row", s)
    col = w.rect("conversation_column", s)
    ok = bool(comp and col) and inside(row or comp, col) and (row or comp)[3] >= 28
    w.check(f"UX {tag}: the composer by numbers", ok, f"composer={comp} row={row} column={col}")
    return ok
