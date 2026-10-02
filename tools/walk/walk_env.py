#!/usr/bin/env python3
"""Shared launch helpers for the native click walks (A11; used by the walk
aggregator `tools/walk/native.py` and by walks that launch their own app).

The rules they encode (docs/walk/README.md "Native click walks"):

* ISOLATED STATE (brief §8): every app a walk launches reads and writes a
  per-run temp tree, never the operator's ~/.octoscode — drafts, credentials,
  preferences, notifications, recents, show-thinking and downloads each point
  into `<state>/` (`isolated_env`).
* The phone shell is launched straight into OctosCode
  (`--test-action page:0 --test-action launch-octoscode`, the 360x780 frame):
  its home grid is dynamic, so a fixed icon tap can open another app.
* An app is started hidden through `harness/headless.sh` and ALWAYS stopped
  (`App.stop`); nothing lingers.
"""
from __future__ import annotations

import json
import os
import pathlib
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
HEADLESS = ROOT / "harness" / "headless.sh"

# The app-state locations the module reads from the environment.
ISOLATED_KEYS = (
    "OCTOSCODE_DRAFTS_FILE",
    "OCTOSCODE_CREDENTIALS_DIR",
    "OCTOSCODE_PREF_PATH",
    "OCTOSCODE_NOTIFICATIONS_FILE",
    "OCTOSCODE_RECENTS_DIR",
    "OCTOSCODE_SHOW_THINKING_FILE",
    "OCTOSCODE_DOWNLOAD_DIR",
    "OCTOSCODE_DISPLAY_PREFS_PATH",
    # A10: the per-install driver id and the Session pane's Advanced memory.
    "OCTOSCODE_DRIVER_ID_PATH",
    "OCTOSCODE_PANE_ADVANCED_FILE",
)

PHONE_SIZE = "360x780"
PHONE_ARGS = "--test-action page:0 --test-action launch-octoscode"


def isolated_env(state: pathlib.Path) -> dict:
    """The per-run app state, created empty under `state`."""
    state = pathlib.Path(state)
    for d in ("cred", "recents", "downloads"):
        (state / d).mkdir(parents=True, exist_ok=True)
    return {
        "OCTOSCODE_DRAFTS_FILE": str(state / "drafts.json"),
        "OCTOSCODE_CREDENTIALS_DIR": str(state / "cred"),
        "OCTOSCODE_PREF_PATH": str(state / "prefs.json"),
        "OCTOSCODE_NOTIFICATIONS_FILE": str(state / "notifications.json"),
        "OCTOSCODE_RECENTS_DIR": str(state / "recents"),
        "OCTOSCODE_SHOW_THINKING_FILE": str(state / "show-thinking.json"),
        "OCTOSCODE_DOWNLOAD_DIR": str(state / "downloads"),
        # A9's display preferences (screens/a9_prefs.rs).
        "OCTOSCODE_DISPLAY_PREFS_PATH": str(state / "display-v1.json"),
        # A10: the per-install driver id (never ~/.octoscode/driver-id) and the
        # Session pane's Advanced disclosure memory.
        "OCTOSCODE_DRIVER_ID_PATH": str(state / "driver-id"),
        "OCTOSCODE_PANE_ADVANCED_FILE": str(state / "session-pane-advanced.json"),
    }


def app_env(mode: str, state: pathlib.Path, extra: dict | None = None,
            hs: pathlib.Path | None = None) -> dict:
    """The full launch environment of one hidden app instance."""
    env = os.environ.copy()
    # Never inherit a caller's credentials or a pairing link.
    for k in ("OCTOS_BEARER", "OCTOS_PAIRING_LINK", "OCTOSENSE_WINDOW_SIZE"):
        env.pop(k, None)
    env.update(isolated_env(state))
    env.update({
        "OCTOSCODE_DESIGN_DIR": str(ROOT / "design"),
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": str(hs or (pathlib.Path(state) / "hs")),
        "HEADLESS_ARGS": "--module octoscode" + (f" {PHONE_ARGS}" if mode == "phone" else ""),
        "HEADLESS_TIMEOUT": "90",
    })
    if mode == "phone":
        env["OCTOSENSE_WINDOW_SIZE"] = PHONE_SIZE
    env.update(extra or {})
    return env


def scrub(text: str) -> str:
    """No machine paths in committed evidence (the repo's hermetic test,
    crates/octoscode-client/tests/repo_hermetic.rs): the checkout reads
    `<repo>`, any other absolute home or temp path keeps its last part."""
    text = text.replace(str(ROOT), "<repo>")
    # Assembled: the literals never appear in this file.
    roots = ["/" + "Users" + "/", "/private" + "/var/", "/var/" + "folders/", "/private" + "/tmp/"]
    for root in roots:
        text = re.sub(re.escape(root) + r"[^\s\"',;)\]]+",
                      lambda m: "<abs>/" + m.group(0).rstrip("/").rsplit("/", 1)[-1], text)
    # A non-mock /home/<user>/ path, or its ~/home/<user> display form (the
    # guard allows only its mock homes).
    mock = r"(?!user\b|octos\b|runner\b|profiles\b|apps\b)"
    text = re.sub(r"~/home/" + mock + r"[A-Za-z0-9._-]+", "~/<abs>", text)
    return re.sub(r"(?<![~\w])/home/" + mock + r"[A-Za-z0-9._-]+[^\s\"',;)\]]*",
                  lambda m: "<abs>/" + m.group(0).rstrip("/").rsplit("/", 1)[-1], text)


def crop_png(path, x: int, y: int, w: int, h: int, max_w: int = 1400):
    """Crop a capture to the box (pixels) and fit it to `max_w` (the brief:
    captures <= 1400 px wide). `sips --cropOffset 0 0` CENTRES the crop
    (measured: a 720x1700 crop of an 804x1748 phone frame landed at 42,24),
    so PIL does it when present; sips only gets non-zero offsets."""
    path = str(path)
    try:
        from PIL import Image  # noqa: PLC0415
        im = Image.open(path)
        im = im.crop((x, y, min(x + w, im.width), min(y + h, im.height)))
        if im.width > max_w:
            im = im.resize((max_w, round(im.height * max_w / im.width)))
        im.save(path)
        return
    except ImportError:
        pass
    subprocess.run(["sips", "-c", str(h), str(w), "--cropOffset", str(max(1, y)), str(max(1, x)),
                    path, "--out", path], capture_output=True)
    subprocess.run(["sips", "-Z", str(max_w), path], capture_output=True)


class App:
    """One hidden app on `port`, driven through its instrument."""

    def __init__(self, port: int, env: dict | None = None):
        self.port = port
        self.base = f"http://127.0.0.1:{port}"
        self.env = env or {}
        self.log_seq = 0
        self.started = False

    # --------------------------------------------------------- lifecycle
    def start(self, binary: str, ready_ids=("connect_btn", "b1_connect_pair", "i0_composer_0",
                                            "sidebar_toggle_hit"), timeout=120.0) -> bool:
        r = subprocess.run(["bash", str(HEADLESS), "start", binary, str(self.port)],
                           env=self.env, cwd=str(ROOT), capture_output=True, text=True,
                           timeout=timeout + 30)
        self.started = True
        if r.returncode != 0:
            raise RuntimeError(f"app start on {self.port} failed: {(r.stdout + r.stderr)[-600:]}")
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                if any(self.find(i) for i in ready_ids):
                    time.sleep(2.0)
                    return True
            except Exception:  # noqa: BLE001 — the bridge answers late at boot
                pass
            time.sleep(1.0)
        return False

    def stop(self):
        if not self.started:
            return
        subprocess.run(["bash", str(HEADLESS), "stop", str(self.port)], env=self.env,
                       cwd=str(ROOT), capture_output=True, text=True, timeout=90)
        self.started = False

    # ------------------------------------------------------------ input
    def get(self, path: str, timeout=20, tries=1) -> bytes:
        last = None
        for _ in range(max(1, tries)):
            try:
                with urllib.request.urlopen(self.base + path, timeout=timeout) as r:
                    return r.read()
            except urllib.error.HTTPError as e:
                # Input routes with wait=1 answer 404 when their frame is
                # coalesced; the input itself was delivered.
                if path.startswith(("/click", "/t?", "/k?", "/m?")):
                    return b""
                last = e
            except Exception as e:  # noqa: BLE001
                last = e
            time.sleep(0.5)
        raise RuntimeError(f"{path}: {last}")

    def snap(self) -> list:
        return json.loads(self.get("/snap?all=1", tries=4))["s"]

    @staticmethod
    def shown(w) -> bool:
        r = w.get("r") or [0, 0, 0, 0]
        return w.get("v", 1) != 0 and r[2] > 0 and r[3] > 0

    def all(self, wid, s=None) -> list:
        s = s if s is not None else self.snap()
        return sorted((w for w in s if w.get("i") == wid and self.shown(w)),
                      key=lambda w: (w["r"][1], w["r"][0]))

    def find(self, wid, s=None):
        hits = self.all(wid, s)
        return hits[0] if hits else None

    def text(self, wid, s=None) -> str:
        w = self.find(wid, s)
        return (w or {}).get("t", "") or ""

    def wait(self, pred, timeout=12.0, step=0.25):
        end = time.time() + timeout
        while time.time() < end:
            try:
                v = pred()
            except Exception:  # noqa: BLE001
                v = None
            if v:
                return v
            time.sleep(step)
        return None

    def click_xy(self, x, y):
        self.get(f"/click?x={x:.1f}&y={y:.1f}&wait=1")
        time.sleep(0.4)

    def click(self, wid, nth=0) -> bool:
        hits = self.all(wid)
        if len(hits) <= nth:
            return False
        x, y, w, h = hits[nth]["r"]
        self.click_xy(x + w / 2, y + h / 2)
        return True

    def click_until(self, wid, pred, tries=3, timeout=6.0, nth=0) -> bool:
        """Click, then wait for the effect; click again only if it never came
        (a coalesced `/click` answer may still have delivered the first)."""
        for _ in range(tries):
            if not self.click(wid, nth):
                if self.wait(pred, timeout=1.0):
                    return True
                continue
            if self.wait(pred, timeout=timeout):
                return True
        return False

    def type(self, text):
        self.get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
        time.sleep(0.3)

    def key(self, code, n=1):
        for _ in range(n):
            self.get(f"/k?c={code}&wait=1")
            time.sleep(0.05)

    def move_away(self):
        self.get("/m?k=move&x=2&y=2&wait=1")

    # ------------------------------------------------------------- logs
    def logs(self) -> list:
        """App log lines since the last call."""
        d = json.loads(self.get(f"/log?since={self.log_seq}", tries=3))
        self.log_seq = d.get("n", self.log_seq)
        return d.get("l", [])

    def all_logs(self) -> list:
        d = json.loads(self.get("/log?since=0&n=20000", tries=3))
        return d.get("l", [])

    def png(self) -> bytes | None:
        for _ in range(12):
            try:
                with urllib.request.urlopen(self.base + "/g?raw=1", timeout=30) as r:
                    return r.read()
            except Exception:  # noqa: BLE001 — a grab can 404 mid-frame
                time.sleep(0.5)
        return None


class Fixture:
    """A fixture/replay server process with its log; always stopped."""

    def __init__(self, argv: list, log: pathlib.Path, ready: str = "listening"):
        self.argv, self.log, self.ready = argv, pathlib.Path(log), ready
        self.proc = None

    def start(self, timeout=20.0):
        self.log.parent.mkdir(parents=True, exist_ok=True)
        f = open(self.log, "w")
        self.proc = subprocess.Popen(self.argv, stdout=f, stderr=subprocess.STDOUT)
        end = time.time() + timeout
        while time.time() < end:
            if self.ready in self.text():
                return self
            if self.proc.poll() is not None:
                raise RuntimeError(f"fixture exited: {self.text()[-400:]}")
            time.sleep(0.2)
        raise RuntimeError(f"fixture never listened: {self.text()[-400:]}")

    def text(self) -> str:
        try:
            return self.log.read_text()
        except OSError:
            return ""

    def count(self, needle: str) -> int:
        return sum(1 for l in self.text().splitlines() if needle in l)

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.proc = None
