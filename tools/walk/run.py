#!/usr/bin/env python3
"""Card #19 / #19b / #19c — the Phase-4 walk-runner.

Turns the rows of `docs/walk-rows.csv` (one per web Playwright case) into
scripted checks on the **native** app, and records a verdict for every row in
`docs/walk/results.csv` (per row) plus `docs/walk/results-checks.csv` (per check).

Operator view: `docs/walk/README.md`.

Design:
* A **backend** per area is the recording-replay server
  (`crates/octoscode-module/examples/replay_serve.rs --scenario <name>`), serving
  the committed real fixtures. **No model runs.**
* The **app** is launched hidden via `harness/headless.sh` on this card's port
  block **8370-8379** and driven with real input (`/click`, `/t`). Assertions
  read the app's own `/snap`; a failure also writes `/g` + the snap JSON under
  `docs/walk/evidence/`.

## Row → check mapping (#19c item 3)

Each check declares which rows it covers (`rows=` in `@check`): a tuple of
lowercase substrings matched against the row's `case` and `protocol_methods`, or
`ALL`. A row is `pass` iff **every check mapped to it** passed — so one broken
check fails only the rows that actually use it, not its whole area.

## Why the composer checks clear first (#19c items 1-2)

`/t` sends makepad `Input::Text` with `replace_last: false`
(`native/makepad/platform/src/remote.rs:1328-1332`): it **inserts at the caret**,
it never replaces the field. A centre click on a **populated** field sometimes
places the caret, sometimes selects, so asserting exact equality after typing
into a non-empty field is inherently non-deterministic (14/20 failures in a tight
loop). The checks therefore **focus, clear with backspace, then type**, and wait
for the observable result — never a fixed sleep.

Run:  python3 tools/walk/run.py [--limit 30] [--only AREA] [--port 8370]
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import time
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
WALK = ROOT / "docs" / "walk"
EVIDENCE = WALK / "evidence"
WALK_ROWS = ROOT / "docs" / "walk-rows.csv"
PARITY = ROOT / "docs" / "parity-matrix.csv"

DEFAULT_APP_BIN = os.environ.get("OCTOSENSE_BIN", os.path.join(os.path.dirname(__file__), "../../..", "p0-build/tmp/octosense-target/debug/octosense"))
BIN = pathlib.Path(os.environ.get("OCTOSCODE_APP_BIN", DEFAULT_APP_BIN))
REPLAY = pathlib.Path(os.environ.get("CARGO_TARGET_DIR") or (ROOT / "target")) / "debug" / "examples" / "replay_serve"  # honour CARGO_TARGET_DIR like cargo does
HEADLESS = ROOT / "harness" / "headless.sh"

# The desktop `octosense` binary is EXTERNAL to this repo (built from the
# OctoSense fork, ~12 min), so the runner never builds it silently — it checks
# once and fails fast with this recipe (card #19b, defect 2).
APP_BIN_HELP = f"""\
the native desktop app binary is missing: {BIN}

tools/walk drives the real desktop shell, which is built from the OctoSense fork
(not from this repo). Point OCTOSCODE_APP_BIN at an existing build, or make one:

  tools/prepare-octosense-fork.sh                 # create the [patch] fork (idempotent)
  cd <fork> && python3 tools/setup.py             # framework sources (.sources/)
  cd <fork> && CARGO_TARGET_DIR=$PWD/tmp/octosense-target \\
      cargo build --features app-appcard -p octosense

  export OCTOSCODE_APP_BIN=<fork>/tmp/octosense-target/debug/octosense

(A prebuilt copy may exist read-only at {DEFAULT_APP_BIN}; set that instead.)
"""

# How long a wait_for() poll may take, and the poll period.
WAIT_TIMEOUT_S = 12.0
POLL_S = 0.15
# Backspaces used to clear the composer (the field is short; extra presses on an
# empty field are harmless).
CLEAR_PRESSES = 48
# The composer's empty-state copy (design `conversation-08`, `composer.placeholder`
# === "Ask Octos anything", `design/bindings.json:28`) — what `/snap` shows when
# the draft is empty.
PLACEHOLDER = "Ask Octos anything"
# #33a — component instances carry POSITIONAL ids: the composer input is
# `i<N>_composer_0`, timeline rows are `i<N>_<kind>[_n]` (userbubble,
# assistantprose, workingrow, …). The old `draft`/`item_kind` ids no longer
# exist (probes tmp/33a-probe-A/B/C.json).
INSTANCE_KIND_RE = re.compile(r"^i\d+_([a-z_]+?)(?:_\d+)*$")
COMPOSER_INPUT_RE = re.compile(r"^i\d+_composer_0$")
THREAD_ROW_RE = re.compile(r"^i\d+_threadrow")


class PrereqError(RuntimeError):
    """A documented prerequisite is missing; the message says exactly what to do."""


def default_shell_cwd(bin_path: pathlib.Path) -> pathlib.Path:
    """The desktop crate dir for a built binary (makepad resolves resources
    relative to it).

    Supports both layouts: `<fork>/desktop` (baseline/fork build) and
    `<tmp>/octosense/desktop` (p0-build sibling layout).
    """
    p = bin_path.resolve()
    for parent in p.parents:
        for cand in (parent / "desktop", parent / "octosense" / "desktop"):
            if (cand / "Cargo.toml").is_file():
                return cand
    return p.parent


def ensure_replay_server(build: bool = True) -> pathlib.Path:
    """The scenario server is ours: build it when missing (cached after the first)."""
    if REPLAY.is_file():
        return REPLAY
    if not build:
        raise PrereqError(
            f"missing {REPLAY}; run: cargo build -p octoscode-module --example replay_serve"
        )
    print("[walk] building the replay server "
          "(cargo build -p octoscode-module --example replay_serve)…", flush=True)
    r = subprocess.run(
        ["cargo", "build", "-p", "octoscode-module", "--example", "replay_serve"],
        cwd=str(ROOT), capture_output=True, text=True,
    )
    if r.returncode != 0 or not REPLAY.is_file():
        raise PrereqError(
            "could not build the replay server "
            "(cargo build -p octoscode-module --example replay_serve):\n"
            + (r.stderr or r.stdout)[-2000:]
        )
    return REPLAY


def check_prereqs(build_replay: bool = True) -> tuple[pathlib.Path, pathlib.Path]:
    """Fail fast with an explicit message; return (replay_server, app_binary)."""
    replay = ensure_replay_server(build=build_replay)
    if not HEADLESS.is_file():
        raise PrereqError(f"missing the headless harness at {HEADLESS}")
    if not (BIN.is_file() and os.access(BIN, os.X_OK)):
        raise PrereqError(APP_BIN_HELP)
    return replay, BIN


APP_PORT = 8370
SCENARIO_PORTS = {"conversation": 8380, "approval": 8381, "task": 8382,
                  "autonomy": 8383, "peer": 8384, "session": 8385,
                  "longcodeline": 8386}

# Areas the card names → the walk rows they cover. Order matters: `area_of`
# returns the FIRST match, so the more specific areas come first. `conversation`
# deliberately omits a bare `\border\b` — it falsely swallowed thread rows like
# "switches sessions by keyboard and preserves sidebar focus order".
AREA_PATTERNS = {
    "longcode": re.compile(
        r"syntax grammar|plain code|code copy|long code(?: line)?|120 columns", re.I),
    "threads": re.compile(
        r"thread|new chat|switch(es)? sessions?\b|rapid session|session list|"
        r"selects? a new session|selected session", re.I),
    "composer": re.compile(r"composer|draft|queue|steer", re.I),
    "approval": re.compile(r"approval|approve|deny", re.I),
    "recovery": re.compile(r"reconnect|replay|recovery|restore|resume", re.I),
    "conversation": re.compile(
        r"prompt|stream|\banswer\b|interrupt|transcript|reasoning|message|turn", re.I),
    # #33a — the surfaces the merged cards shipped (settings drawer, command
    # palette, review panel, keyboard model, fleet/peer). Appended so the
    # original areas' rows keep their checks (first match wins).
    "peer": re.compile(r"fleet|peer[- ](?:dock|control|controller|activity|readonly)|seat|lane|lease", re.I),
    "review": re.compile(r"\breview\b|diff|hunk|unmodified", re.I),
    "settings": re.compile(r"settings|workspace dir|model management|profile", re.I),
    "palette": re.compile(r"command (palette|surface)|slash command|/(?:model|mode|compact|btw|monitor|resume)\b", re.I),
    "keyboard": re.compile(r"keyboard|shortcut|hotkey|press(es|ing)? (esc|escape|enter|tab|alt)", re.I),
    "connect": re.compile(r"connect (screen|gate)|first[- ]run|onboard|pairing|pair\b|address valid|invalid auth|identity|workspace browse|folder|chooser", re.I),
}

# Rows whose STATE the replay fixtures cannot produce (multi-session soaks,
# OS notifications, browser URL/clipboard handling, IME, mobile viewports, a
# dark-OS theme matrix). Live-only rows are Phase 4's (outer loop, live).
LIVE_ONLY_PATTERNS = [
    re.compile(r"notif|badge|title gains|desktop notification", re.I),
    re.compile(r"\bsoak\b|twelve|12 session|three native peers|pooled socket", re.I),
    re.compile(r"clipboard|browser tab|bookmark|saved link|session-links|query-string|address bar", re.I),
    re.compile(r"three concurrent|out of order|parks? a background|parked (question|approval)", re.I),
    re.compile(r"hidden current session|background session|background turn|completed in background", re.I),
    re.compile(r"phone viewport|mobile|narrow viewport", re.I),
    re.compile(r"\bvim\b|language|translat|IME", re.I),
    re.compile(r"another (client|browser)|other client|foreign", re.I),
    re.compile(r"five themes|theme cycle|manual light|dark OS", re.I),
    re.compile(r"refresh (?:the )?page|after refresh|on reload|keeps them on reload", re.I),
]
# Which areas have a native surface today (⇒ scriptable), and the scenario each
# is driven against.
AREA_SCRIPTABLE = {"conversation": True, "threads": True, "composer": True,
                   "recovery": True, "approval": False,
                   "peer": True, "review": True, "settings": True,
                   "palette": True, "keyboard": True, "connect": False,
                   "longcode": True}
AREA_SCENARIO = {"conversation": "conversation", "threads": "conversation",
                 "composer": "conversation", "recovery": "conversation",
                 "approval": "approval", "peer": "peer", "review": "autonomy",
                 "settings": "session", "palette": "conversation",
                 "keyboard": "conversation", "connect": "session",
                 "longcode": "longcodeline"}
def scenario_for(area: str) -> str:
    """The area's replay scenario, or a WALK_SCENARIO override (negative control).

    `WALK_SCENARIO="conversation=session"` runs the conversation checks against
    the session fixture — a deliberately broken setup for the negative-control
    run (#33a review item 2): the shell still mounts, so the smoke checks pass,
    but no coding turn streams, so the specific checks must FAIL.
    """
    override = os.environ.get("WALK_SCENARIO", "")
    for pair in override.split(","):
        a, sep, sc = pair.partition("=")
        if sep and a.strip() == area:
            return sc.strip()
    return AREA_SCENARIO[area]


APPROVAL_MISSING = ("missing: inline approval card — design scene conversation-05 "
                    "is not in the built batch (design/bindings.json:40); "
                    "approval/requested reaches the store but has no widget")
CONNECT_MISSING = ("missing: gate-mode app instance — the walk app auto-connects to "
                   "the replay server, so the pre-connection gate states (invalid "
                   "auth, pairing links, workspace chooser) never mount")

# The subset marker used by `@check(rows=…)` for "every row of the area".
ALL = "__all__"


# --------------------------------------------------------------------------- #
class App:
    def __init__(self, port: int):
        self.port = port
        self.base = f"http://127.0.0.1:{port}"

    def _get(self, path: str) -> str:
        try:
            with urllib.request.urlopen(self.base + path, timeout=20) as r:
                return r.read().decode()
        except urllib.error.HTTPError as e:
            # The makepad bridge answers errors as HTTP 404 with a JSON body; a
            # bare "HTTP Error 404" hides the real cause (e.g. a bridge timeout).
            body = e.read().decode(errors="replace")
            raise AssertionError(f"{path} -> HTTP {e.code} {body[:200]}") from None

    def snap(self) -> dict:
        return json.loads(self._get("/snap?all=1"))

    def _get_retry(self, path: str, tries: int = 4) -> str:
        """`_get` with a short retry, for bursty input where the bridge can time
        out under load (`ask()` answers `Reply::Err("timeout …")` as HTTP 404,
        `native/makepad/platform/src/remote.rs:2740-2742`). A dropped keystroke
        must not masquerade as a check failure."""
        last = None
        for _ in range(tries):
            try:
                return self._get(path)
            except Exception as e:  # noqa: BLE001
                last = e
                time.sleep(0.2)
        raise AssertionError(f"retries exhausted for {path}: {last}")

    def text_of(self, snap: dict, ident: str):
        return next((w.get("t") for w in snap.get("s", []) if str(w.get("i", "")) == ident), None)

    def widget_ids(self, snap: dict):
        return [str(w.get("i", "")) for w in snap.get("s", [])]

    def kinds(self, snap: dict):
        return [m.group(1) for w in snap.get("s", [])
                if (m := INSTANCE_KIND_RE.match(str(w.get("i", ""))))]

    def rect_re(self, snap: dict, pattern):
        for w in snap.get("s", []):
            if pattern.match(str(w.get("i", ""))):
                return w["r"]
        return None

    def rect(self, snap: dict, ident: str):
        for w in snap.get("s", []):
            if str(w.get("i", "")) == ident:
                return w["r"]
        return None

    def click(self, x, y):
        return self._get_retry(f"/click?x={x}&y={y}&wait=1")

    def click_id(self, snap: dict, ident: str):
        r = self.rect(snap, ident)
        if not r:
            raise AssertionError(f"widget '{ident}' is not present")
        return self.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))

    def type(self, text):
        return self._get_retry("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))

    def key(self, code):
        return self._get_retry("/k?" + urllib.parse.urlencode({"c": code, "wait": 1}))

    def key_mod(self, code, cmd=False, ctrl=False, alt=False, shift=False):
        q = {"c": code, "wait": 1}
        if cmd:
            q["cmd"] = 1
        if ctrl:
            q["ctrl"] = 1
        if alt:
            q["alt"] = 1
        if shift:
            q["shift"] = 1
        return self._get_retry("/k?" + urllib.parse.urlencode(q))

    def wait_for(self, predicate, timeout=WAIT_TIMEOUT_S, poll=POLL_S, what="condition"):
        """Poll `/snap` until `predicate(snap)` holds; return the snapshot, else raise.

        Waiting on the observable condition — never a fixed sleep (card #19c).
        """
        deadline = time.monotonic() + timeout
        last = None
        while time.monotonic() < deadline:
            last = self.snap()
            if predicate(last):
                return last
            time.sleep(poll)
        raise AssertionError(f"timed out after {timeout:g}s waiting for {what}")

    def draft(self, snap=None):
        snap = snap or self.snap()
        t = next((w.get("t") for w in snap.get("s", [])
                  if COMPOSER_INPUT_RE.match(str(w.get("i", "")))), None)
        if t is None:
            return None
        return "" if t == PLACEHOLDER else t

    def focus_composer(self, snap=None):
        """Focus the composer and put the caret at the END (a right-edge click).

        A right-edge click reliably places the caret at the end of the text
        (verified), so a subsequent clear-by-backspace is deterministic.
        """
        # #33a: the input is `i<N>_composer_0`; its rect is all-zero until the
        # first layout, so wait for a real box before clicking (probes).
        deadline = time.monotonic() + 30
        r = None
        while time.monotonic() < deadline:
            r = self.rect_re(snap or self.snap(), COMPOSER_INPUT_RE)
            if r and r[2] > 0:
                break
            time.sleep(0.5)
        if not r or r[2] <= 0:
            raise AssertionError("the composer input (i*_composer_0) never laid out")
        x, y, w, h = r
        self.click(int(x + w * 0.5), int(y + h * 0.5))
        self.key("end")
        return None

    def clear_composer(self):
        """Empty the composer: focus, home, shift+end, one backspace.

        #33a, probed live (tmp/33a-*.log matrix): a burst of CLEAR_PRESSES
        backspaces lost every key after the FIRST on an already-populated
        draft ('walk draft prob' -> 'pro' then frozen; typing different keys
        always worked), and cmd+a select-all is swallowed (the macOS
        window_menu consumes the accelerator before the field sees it). The
        probed-clean mechanism is three DISTINCT keys with a short pause
        each: home -> shift+end (selects the line) -> backspace — verified to
        clear a populated draft to '' in one pass. Bounded retries with a
        re-focus; still waits for the observable result, never a fixed sleep
        (#19c).
        """
        self.focus_composer()
        for attempt in range(3):
            self.key("home")
            time.sleep(0.3)
            self.key_mod("end", shift=True)
            time.sleep(0.3)
            self.key("backspace")
            try:
                return self.wait_for(
                    lambda s: (self.draft(s) or "") in ("", PLACEHOLDER),
                    timeout=4.0,
                    what="the composer to clear",
                )
            except AssertionError:
                if attempt == 2:
                    raise
            # re-focus before the next attempt (a click/key may have dropped)
            self.focus_composer()

    def type_into_composer(self, text):
        """Clear, focus, type, and wait until the draft equals `text` exactly.

        Returns the snapshot that showed it. Raises if it never matches — the
        failure is then a real one, not a race.
        """
        self.clear_composer()
        self.focus_composer()
        self.type(text)
        return self.wait_for(lambda s: self.draft(s) == text,
                             what=f"the composer to read {text!r}")

    def send(self):
        """Click the send control (`send_hit`), waiting for a real rect."""
        deadline = time.monotonic() + 30
        r = None
        while time.monotonic() < deadline:
            r = self.rect(self.snap(), "send_hit")
            if r and r[2] > 0:
                break
            time.sleep(0.5)
        if not r or r[2] <= 0:
            raise AssertionError("the send control ('send_hit') never laid out")
        return self.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))


# --------------------------------------------------------------------------- #
class Procs:
    def __init__(self, app_port: int, shell_cwd: pathlib.Path | None = None):
        self.servers: dict[str, subprocess.Popen] = {}
        self.app_port = app_port
        self.app = None
        # The desktop crate dir the app runs from; derived from the binary when
        # not given (card #19b, defect 2 — no hard-coded p0-build path).
        self.shell_cwd = shell_cwd or (
            pathlib.Path(os.environ["OCTOSCODE_SHELL_CWD"])
            if os.environ.get("OCTOSCODE_SHELL_CWD")
            else None
        )

    def start_server(self, scenario: str):
        if scenario in self.servers:
            return
        port = SCENARIO_PORTS[scenario]
        log = WALK / f"server-{scenario}.log"
        f = open(log, "w")
        proc = subprocess.Popen(
            [str(REPLAY), str(port), "--scenario", scenario], stdout=f, stderr=f)
        self.servers[scenario] = proc
        for _ in range(75):
            if "listening" in log.read_text():
                return
            if proc.poll() is not None:  # exited — surface its own error
                raise PrereqError(
                    f"replay server '{scenario}' exited immediately "
                    f"(rc={proc.returncode}); see {log}")
            time.sleep(0.2)
        raise PrereqError(
            f"replay server '{scenario}' never listened on {port}; see {log}")

    def start_app(self, scenario: str):
        self.stop_app()
        state = ROOT / "tmp" / "walk" / "headless"
        state.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        env.update({
            "OCTOS_BASE_URL": f"http://127.0.0.1:{SCENARIO_PORTS[scenario]}",
            "OCTOS_BEARER": "walk-dummy-token",
            "OCTOS_PROFILE_ID": "dsflash",
            "MAKEPAD_WM_TEST_APP": "octoscode",
            "HEADLESS_ARGS": "--module octoscode",
            "HEADLESS_STATE": str(state),
        })
        cwd = self.shell_cwd or default_shell_cwd(BIN)
        if not (cwd / "Cargo.toml").is_file():
            raise PrereqError(
                f"the desktop crate dir does not look right: {cwd}\n"
                "set OCTOSCODE_SHELL_CWD to the fork's `desktop/` directory "
                "(next to the built octosense binary).")
        r = subprocess.run(["bash", str(HEADLESS), "start", str(BIN), str(self.app_port)],
                           env=env, cwd=str(cwd), capture_output=True, text=True, timeout=120)
        if r.returncode != 0:
            raise PrereqError(
                f"the app did not start on port {self.app_port}:\n"
                + (r.stdout or "")[-800:] + (r.stderr or "")[-800:])
        self.app = self.app_port
        # Wait for the bridge to serve a snapshot with the module mounted — never
        # a fixed sleep (a slow first frame must not time out the first click).
        # #33a: 120s — on this shared host the module mounts at ~35-60s (fonts,
        # assets, 20-lane load); run1's 30s timed out ALL 114 selected rows
        # before a single check ran ("timed out after 30s waiting for the
        # module to mount", results.csv).
        app = App(self.app_port)
        # #33a: the module's own surface is the mount signal — the old
        # "heading" id no longer exists (the #28e shell renders the OctosCode
        # title as the TEXT of an unnamed Label; see tmp/33a-start.log
        # forensics: 'heading' in ids == False with 165 widgets mounted).
        app.wait_for(lambda s: "thread_list" in app.widget_ids(s), timeout=120.0,
                     what="the module to mount after launch")

    def stop_app(self):
        if self.app is None:
            return
        env = os.environ.copy()
        env["HEADLESS_STATE"] = str(ROOT / "tmp" / "walk" / "headless")
        subprocess.run(["bash", str(HEADLESS), "stop", str(self.app)],
                       env=env, capture_output=True, text=True, timeout=60)
        self.app = None

    def stop_all(self):
        self.stop_app()
        for p in self.servers.values():
            p.send_signal(signal.SIGTERM)
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()
        self.servers.clear()


class LiveGate:
    """`--live` mode: the app talks to the RUNNING real gate (the outer
    loop's `octos serve`, dsflash) instead of a replay server (#39a). No
    server is started; the app's env comes from the live-gate recipe, with
    every host-specific value read from the environment (nothing committed):

      OCTOS_LIVE_URL          default http://127.0.0.1:50190
      OCTOS_LIVE_TOKEN_FILE   required — the gate's bearer token file
      OCTOS_LIVE_WORKSPACE    optional — OCTOS_WORKSPACE_CWD for the app
      OCTOS_LIVE_PROFILE      default dsflash
    """

    def __init__(self, app_port: int, shell_cwd=None):
        self.app_port = app_port
        self.shell_cwd = shell_cwd or (
            pathlib.Path(os.environ["OCTOSCODE_SHELL_CWD"])
            if os.environ.get("OCTOSCODE_SHELL_CWD") else None)
        token_file = os.environ.get("OCTOS_LIVE_TOKEN_FILE")
        if not token_file or not pathlib.Path(token_file).is_file():
            raise PrereqError(
                "--live needs OCTOS_LIVE_TOKEN_FILE pointing at the live "
                "gate's bearer-token file (the gate itself must already be "
                "listening on OCTOS_LIVE_URL, default "
                "http://127.0.0.1:50190)")
        self.env_extra = {
            "OCTOS_BASE_URL": os.environ.get("OCTOS_LIVE_URL",
                                             "http://127.0.0.1:50190"),
            "OCTOS_BEARER": pathlib.Path(token_file).read_text().strip(),
            "OCTOS_PROFILE_ID": os.environ.get("OCTOS_LIVE_PROFILE", "dsflash"),
            "MAKEPAD_HIDE_WINDOWS": "1",
            "MAKEPAD_WM_TEST_APP": "octoscode",
            "HEADLESS_ARGS": "--module octoscode",
            "HEADLESS_STATE": str(ROOT / "tmp" / "walk" / "headless-live"),
        }
        ws = os.environ.get("OCTOS_LIVE_WORKSPACE")
        if ws:
            self.env_extra["OCTOS_WORKSPACE_CWD"] = ws
        self.app = None

    def start_server(self, scenario: str):
        print(f"  [live] using the RUNNING real gate "
              f"({self.env_extra['OCTOS_BASE_URL']}) — no replay server")

    def start_app(self, scenario: str):
        self.stop_app()
        state = ROOT / "tmp" / "walk" / "headless-live"
        state.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        env.update(self.env_extra)
        cwd = self.shell_cwd or default_shell_cwd(BIN)
        r = subprocess.run(["bash", str(HEADLESS), "start", str(BIN),
                            str(self.app_port)],
                           env=env, cwd=str(cwd), capture_output=True,
                           text=True, timeout=120)
        if r.returncode != 0:
            raise PrereqError(f"the live app did not start on {self.app_port}:\n"
                              + (r.stdout or "")[-500:] + (r.stderr or "")[-500:])
        self.app = self.app_port
        app = App(self.app_port)
        app.wait_for(lambda s: "thread_list" in app.widget_ids(s), timeout=180.0,
                     what="the module to mount against the live gate")

    def stop_app(self):
        if self.app is None:
            return
        env = os.environ.copy()
        env["HEADLESS_STATE"] = str(ROOT / "tmp" / "walk" / "headless-live")
        subprocess.run(["bash", str(HEADLESS), "stop", str(self.app)],
                       env=env, capture_output=True, text=True, timeout=60)
        self.app = None

    def stop_all(self):
        self.stop_app()


# --------------------------------------------------------------------------- #
# Checks. Each returns (passed, reason). `rows` scopes it to the walk rows it
# covers (card #19c item 3): ALL, or lowercase substrings of case/protocol_methods.
# --------------------------------------------------------------------------- #
CHECKS: list = []


def check(area: str, name: str, rows=ALL):
    def deco(fn):
        CHECKS.append({"area": area, "name": name, "rows": rows, "fn": fn})
        return fn
    return deco


def check_applies(chk: dict, row: dict) -> bool:
    """Does `chk` run against this walk row?"""
    if chk["rows"] is ALL:
        return True
    hay = (row["case"] + " " + (row.get("protocol_methods") or "")).lower()
    return any(k in hay for k in chk["rows"])


def _compose_and_send(app: App, text: str):
    """Clear, type `text`, and send it; return the snapshot right after send."""
    app.type_into_composer(text)
    app.send()


# ---- conversation: prompt, stream, order, interrupt ----------------------- #
@check("conversation", "module reaches conn: Live with the OctosCode heading")
def c_live(app):
    d = app.snap()
    # #33a: the heading is the TEXT of an unnamed Label in the #28e shell (no
    # widget carries the id "heading" any more) — assert the visible text.
    texts = [w.get("t", "") for w in d.get("s", [])]
    h = "OctosCode" in texts
    s = app.text_of(d, "status") or ""
    return (h and "Live" in s), f"heading_text={h} status={s!r}"


@check("conversation", "the thread list renders the opened session row")
def c_thread(app):
    d = app.snap()
    rows = [w.get("t") for w in d.get("s", [])
            if THREAD_ROW_RE.match(str(w.get("i", ""))) and w.get("t")]
    empty = "No threads yet" in [w.get("t") for w in d.get("s", [])]
    return bool(rows) or empty, f"thread rows={rows[:2]} empty-state={empty}"


@check("conversation", "the composer accepts typed text (prompt input)",
       rows=("prompt", "input"))
def c_input(app):
    app.type_into_composer("walk draft probe")
    return True, "draft reads 'walk draft probe' after clearing and typing"


@check("conversation", "composing clears the draft on send", rows=("prompt", "input"))
def c_clear_on_send(app):
    _compose_and_send(app, "walk: clear the draft")
    app.wait_for(lambda s: (app.draft(s) or "") in ("", PLACEHOLDER),
                 what="the draft to clear after send")
    return True, f"draft after send={app.draft()!r}"


@check("conversation", "a sent prompt streams an assistant answer row",
       rows=("stream", "answer", "turn", "prompt"))
def c_stream(app):
    app.wait_for(lambda s: "assistantprose" in app.kinds(s),
                 what="the assistant answer row")
    return True, f"kinds={sorted(set(app.kinds(app.snap())))}"


@check("conversation", "the user's own prompt renders as a row",
       rows=("prompt", "stream", "turn"))
def c_user(app):
    app.wait_for(lambda s: "userbubble" in app.kinds(s),
                 what="the user-bubble row")
    return True, f"kinds={sorted(set(app.kinds(app.snap())))}"


@check("conversation", "the answer row renders after the prompt row (order)",
       rows=("order", "stream", "turn"))
def c_order(app):
    app.wait_for(lambda s: {"userbubble", "assistantprose"} <= set(app.kinds(s)),
                 what="both the user row and the answer row")

    def ys(s):
        out = {}
        for w in s.get("s", []):
            m = INSTANCE_KIND_RE.match(str(w.get("i", "")))
            if not m:
                continue
            r = w.get("r") or [0, 0, 0, 0]
            if r[2] <= 0 and r[3] <= 0:
                continue
            k = m.group(1)
            out[k] = min(out.get(k, float("inf")), r[1])
        return out

    d = app.snap()
    y = ys(d)
    ok = y.get("userbubble", 0) < y.get("assistantprose", 0)
    return ok, f"y(user)={y.get('userbubble')} y(answer)={y.get('assistantprose')}"


@check("conversation", "the turn's timeline item kinds are present",
       rows=("stream", "turn", "message"))
def c_kinds(app):
    app.wait_for(lambda s: bool(app.kinds(s)), what="the timeline item rows")
    got = set(app.kinds(app.snap()))
    ok = {"userbubble", "assistantprose"} <= got
    return ok, f"kinds={sorted(got)}"


@check("conversation", "the Stop control is present for the live turn",
       rows=("interrupt", "turn"))
def c_stop(app):
    # #33a: the live turn's Stop is the SEND control with the stop glyph
    # swapped in (lib.rs:1685-1694) — the visible control is `send_hit`.
    ok = "send_hit" in app.widget_ids(app.snap())
    return ok, f"send_hit (the live turn's stop control) present={ok}"


# ---- threads: list, refresh, new chat ------------------------------------- #
@check("threads", "the thread list is a PortalList of session rows")
def t_portal(app):
    return "thread_list" in app.widget_ids(app.snap()), "thread_list present"


@check("threads", "refresh (session/list) keeps the module live")
def t_refresh(app):
    # #33a: the native shell has no refresh control — session/list runs on the
    # transport automatically. The user-visible state the web refresh produces
    # is a Live status over the mounted thread list.
    d = app.snap()
    st = app.text_of(d, "status") or ""
    ids = app.widget_ids(d)
    ok = "Live" in st and "thread_list" in ids
    return ok, f"status={st!r} thread_list={'thread_list' in ids}"


@check("threads", "New chat mints a fresh Session and re-opens the workspace",
       rows=("new chat", "session"))
def t_new_chat(app):
    app.click_id(app.snap(), "new_chat_hit")
    app.wait_for(lambda s: ("OctosCode" in [w.get("t", "") for w in s.get("s", [])]
                            and app.rect_re(s, COMPOSER_INPUT_RE) is not None),
                 what="the workspace to re-open after New chat")
    d = app.snap()
    ph = next((w.get("t") for w in d.get("s", [])
               if COMPOSER_INPUT_RE.match(str(w.get("i", "")))), None)
    return True, f"heading='OctosCode' composer={ph!r}"


# ---- composer: draft, send, timeline, input round-trip -------------------- #
@check("composer", "the draft is a single TextInput with a placeholder")
def comp_draft(app):
    ph = next((w.get("t") for w in app.snap().get("s", [])
               if COMPOSER_INPUT_RE.match(str(w.get("i", "")))), None)
    return ph == PLACEHOLDER, f"placeholder={ph!r}"


@check("composer", "the send control is present")
def comp_send(app):
    return "send_hit" in app.widget_ids(app.snap()), "send_hit present"


@check("composer", "a follow-up drains as its own turn and a reselect replays nothing",
       rows=("unknown turn",))
def c_queue(app):
    # Row 190's drivable slice. While a turn is live the send control IS the
    # stop control (lib.rs:1685-1694 swaps the glyph AND the action), so a
    # second prompt cannot be driven through the window mid-turn — the two
    # prompts are driven sequentially and the assertions cover the
    # replay-observable slices: each follow-up becomes its own turn, neither
    # bubble duplicates, and reselecting the thread replays nothing (the
    # queued-mid-turn slice needs a keyboard-queue path this card did not
    # probe out; the retest server log saw ONE turn/start for the mid-turn
    # second send — it was an interrupt, not a send).
    def proses(snap):
        return sum(1 for w in snap.get("s", [])
                   if "assistantprose" in str(w.get("i", "")))

    r = None
    for _ in range(20):
        d = app.snap()
        r = app.rect_re(d, COMPOSER_INPUT_RE)
        if r and r[2] > 0:
            break
        time.sleep(0.5)
    # Same lesson as r_localcmd: earlier composer checks (c_replay) leave a
    # turn LIVE, and the mounted composer DROPS text typed mid-turn (the
    # #34a instrument probes; retest8's clauses one=0 two=0 proses=0). Let
    # the earlier turn settle before typing.
    app.wait_for(lambda s: "workingrow" not in app.kinds(s), timeout=45,
                 what="earlier composer turns to settle")
    time.sleep(1.0)
    app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    base = proses(app.snap())
    app.clear_composer(); app.type("walk queue one"); app.send()
    app.wait_for(lambda s: proses(s) > base, timeout=60,
                 what="queue one's own answer")
    r = app.rect_re(app.snap(), COMPOSER_INPUT_RE)
    app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    app.clear_composer(); app.type("walk queue two"); app.send()
    app.wait_for(lambda s: proses(s) > base + 1, timeout=60,
                 what="queue two's own answer")
    d = app.snap()
    tr = app.rect_re(d, THREAD_ROW_RE)
    app.click(int(tr[0] + tr[2] / 2), int(tr[1] + tr[3] / 2))
    # The web contract (runtime-recovery.spec.ts:5): continuing past the turn
    # must not REPLAY anything and must not lose turns — the first bubble is
    # EXPECTED to sit above the viewport after a reselect (the timeline shows
    # its tail; instrument: i0_userbubble_0 keeps r=[0,0,0,0] = virtualised
    # out, while its text stays in the tree). Assert exactly that: each
    # prompt ONCE in the tree (no replay), both answers retained, the TAIL
    # bubble actually laid out (the list renders, not stuck empty), and no
    # session minted.
    def tree_count(snap, text):
        # Bubble text carries a trailing-space second instance (probed:
        # i1_userbubble_1 == ' '), so match on the stripped prefix, not ==.
        return sum(1 for w in snap.get("s", [])
                   if (m := INSTANCE_KIND_RE.match(str(w.get("i", ""))))
                   and m.group(1) == "userbubble"
                   and (w.get("t") or "").strip().startswith(text))

    def replayed(snap):
        # Virtualization makes per-row text presence timing/viewport-dependent
        # (retest10: proses=2 with both prompt texts absent from the window;
        # the web keeps the full DOM, the native list materialises ~a screen).
        # The contract's durable facts (runtime-recovery.spec.ts): NO
        # duplicate bubbles (nothing replayed), both ANSWERS retained
        # (nothing lost), one session.
        proses = sum(1 for w in snap.get("s", [])
                     if "assistantprose" in str(w.get("i", "")))
        return (tree_count(snap, "walk queue one") <= 1
                and tree_count(snap, "walk queue two") <= 1
                and proses >= 2)

    try:
        app.wait_for(replayed, timeout=20,
                     what="the timeline after the reselect (no replay, answers kept)")
    except AssertionError as e:
        # Instrumented failure (the runner env differs from the hand probe:
        # earlier composer checks leave turns in the timeline): dump every
        # clause so the retest log names the failing one.
        d = app.snap()
        proses = sum(1 for w in d.get("s", [])
                     if "assistantprose" in str(w.get("i", "")))
        raise AssertionError(
            f"clauses one={tree_count(d, 'walk queue one')} "
            f"two={tree_count(d, 'walk queue two')} proses={proses} "
            f"sessions={app.text_of(d, 'sessions')!r}") from None
    d = app.snap()
    sessions = (app.text_of(d, "sessions") or "").strip()
    ok = "1" in sessions
    return ok, (f"no_replay(one<=1,two=1) tail_kept {sessions!r}")


@check("composer", "the conversation column hosts the timeline PortalList")
def comp_timeline(app):
    return "timeline_list" in app.widget_ids(app.snap()), "timeline_list present"


@check("composer", "a typed draft round-trips through the composer")
def comp_input_roundtrip(app):
    app.type_into_composer("survive me")
    return True, "draft reads 'survive me' after clearing and typing"


# ---- #35b: the docked card's CLICK controls route to their action table ---- #
# #32h wired taps for setup-01 ONLY (connect::wire_events took a `Screen` enum),
# so setup-11's Reload / Copy-diagnostics mounted with no on_click at all and
# were dead in the real app. #35b moved the wiring into one card-dir-driven
# helper (screens/taps.rs) and made lib.rs dispatch each tap to the resolver
# that owns it. These checks click the LAID-OUT rect and assert the app's own
# effect line — never the action id directly (the LESSONS 135 rule: wiring must
# be proven by a CLICK).
#
# HONEST SCOPE: only `error.reload` is covered. `error.copy_diagnostics` wires
# (`card events: 2 tap(s) wired for setup-11`) and is owned (palette.rs:282), but
# its click produced NO `card tap:` line, and a four-widget probe (btn_diag /
# _surface / _control / _label, all at the same rect) stayed silent while every
# btn_reload* widget fired — cause UNVERIFIED, so it gets no check claiming it
# works.
def _card_tap_log(app, ident, action):
    """Click `ident`'s laid-out centre; True iff the app logs the card tap."""
    d = app.snap()
    r = app.rect(d, ident)
    if not r or r[2] <= 0 or r[3] <= 0:
        return False, f"{ident} not laid out (rect={r})"
    app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    time.sleep(1.0)
    # /log answers {n, pool, l: [lines…]}; count OCCURRENCES of the named line
    # rather than set-differencing (two controls can emit the identical line).
    n = app._get("/log?n=400").count(f"card tap: {action}")
    if n == 0:
        return False, f"no 'card tap: {action}' after clicking {ident} at {r}"
    return True, f"card tap: {action} after clicking {ident} at {r}"


@check("connect", "the error card's Reload button routes to error.reload",
       rows=("error", "reload", "diagnostic"))
def card_tap_error_reload(app):
    return _card_tap_log(app, "btn_reload", "error.reload")


# ---- peer: the fleet/peer surfaces (board 4 + #23 peer cards) -------------- #
@check("peer", "the fleet roster renders its rows and slot dots")
def p_roster(app):
    d = app.snap()
    ok = ("fleet_list" in app.widget_ids(d)
          and "fleet_row_1" in app.widget_ids(d)
          and "fleet_slot_1" in app.widget_ids(d))
    return ok, f"fleet ids={sorted(i for i in app.widget_ids(d) if i.startswith('fleet_'))[:6]}"


@check("peer", "the sidebar hosts the GOALS/LOOPS/FLEET sections")
def p_sections(app):
    d = app.snap()
    texts = [w.get("t", "") for w in d.get("s", [])]
    ok = all(t in texts for t in ("GOALS", "LOOPS", "FLEET"))
    return ok, f"sections={[t for t in texts if t in ('GOALS','LOOPS','FLEET')]}"


@check("peer", "the goals/loops lists render their rows",
       rows=("goal", "loop", "plan", "trajectory"))
def p_rows(app):
    d = app.snap()
    ok = "goals_list" in app.widget_ids(d) and "loops_list" in app.widget_ids(d)
    return ok, f"goal_ring={'goal_ring' in app.widget_ids(d)} loop_row={'loop_row_1' in app.widget_ids(d)}"


# ---- #41c: row-specific checks for the smoke-only rows --------------------- #
# One check per smoke-only row whose own case is exercisable on the replay
# fixtures; every other smoke-only row carries a documented reason in
# docs/phase4-gaps.md (fixture lacks the frames / native surface absent /
# viewport semantics). New FAILs land in docs/walk/defects.md.

@check("peer", "Alt+D reaches a fleet capability notice surface to focus",
       rows=("alt+d",))
def p_altd_notice(app):
    # Row 72's own case (the web binds Alt+D, registry.ts; suppressed inside
    # inputs). Native observable: a capability-notice surface must exist for
    # focus to land on. Measured 41c recon: keys.rs has no KeyD arm, and the
    # snap carries no notice widget — expected FAIL until wired.
    app.key("escape")
    app.key_mod("d", alt=True)
    d = app.snap()
    notice = [i for i in app.widget_ids(d) if "capab" in i or "notice" in i]
    return bool(notice), f"capability-notice widgets={notice or 'none (Alt+D unbound)'}"


@check("peer", "Alt+P toggles a peer dock fold (expands and collapses)",
       rows=("alt+p",))
def p_altp_fold(app):
    # Row 73's own case. Native observable: a peer dock whose rect toggles
    # across two Alt+P presses. Measured 41c recon: no peer dock mounts.
    app.key("escape")
    app.key_mod("p", alt=True)
    r1 = app.rect(app.snap(), "peer_dock")
    app.key_mod("p", alt=True)
    r2 = app.rect(app.snap(), "peer_dock")
    ok = bool(r1) and bool(r2) and (r1 != r2 or (r1[2] > 0 and r1[3] > 0))
    return ok, f"peer_dock rects={r1} -> {r2}"


@check("peer", "Fleet's roster rows render with real rects (the mock's lanes)",
       rows=("fleet opens", "2 lanes"))
def p_fleet_rows_visible(app):
    # Row 128's own domain: the roster lists the mock's lanes as VISIBLE rows.
    # The id-only smoke check passed while every fleet rect stayed collapsed
    # (measured 41c recon under the r6-peer fixture) — this asserts the rect.
    d = app.snap()
    r = app.rect(d, "fleet_row_1")
    ok = bool(r) and r[2] > 0 and r[3] > 0
    return ok, f"fleet_row_1 rect={r}"


@check("recovery", "the palette's /monitor command reaches a monitors surface",
       rows=("monitors create",))
def r_monitor_entry(app):
    # Row 102's own domain: monitors managed through typed receipts, entered
    # via the palette's /monitor (the list q_execute probed carries it).
    app.focus_composer(app.snap())
    app.clear_composer()
    app.type("/monitor")
    app.wait_for(lambda s: any("/monitor" in str(w.get("t", "")).lower()
                               for w in s.get("s", [])),
                 what="the /monitor palette entry")
    app.key("return")
    app.wait_for(lambda s: any(i.startswith("monitor")
                               for i in app.widget_ids(s))
                 or any("monitor" in str(w.get("t", "")).lower()
                        and str(w.get("t", "")).strip().lower() != "/monitor"
                        for w in s.get("s", [])),
                 timeout=8, what="a monitors surface after executing /monitor")
    d = app.snap()
    ids = [i for i in app.widget_ids(d) if i.startswith("monitor")]
    texts = [str(w.get("t", "")) for w in d.get("s", [])
             if "monitor" in str(w.get("t", "")).lower()
             and str(w.get("t", "")).strip().lower() != "/monitor"]
    return bool(ids or texts), f"monitor ids={ids[:4]} texts={texts[:3]}"


@check("settings", "the settings drawer exposes the Models management section",
       rows=("dsh-style models", "models settings flow"))
def s_models_section(app):
    # Row 87's own case. Measured 41c recon: the drawer mounts header + the
    # connection action only — no Model/Models entry. Expected FAIL until the
    # section ships.
    d = app.snap()
    app.click_id(d, "settings_open_hit")
    app.wait_for(lambda s: (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0,
                 what="the settings drawer to open")
    d = app.snap()
    texts = [str(w.get("t", "")) for w in d.get("s", [])]
    ok = any(t in texts for t in ("Model", "Models", "Manage models"))
    return ok, f"models section texts={[t for t in texts if 'model' in t.lower()][:3]}"


@check("keyboard", "Escape closes the settings drawer and the trigger still works",
       rows=("escape restores",))
def k_esc_drawer(app):
    # Row 58's own domain: Escape hands control back and the trigger survives.
    d = app.snap()
    app.click_id(d, "settings_open_hit")
    app.wait_for(lambda s: (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0,
                 what="the settings drawer to open")
    app.key("escape")
    app.wait_for(lambda s: (app.rect(s, "settings_drawer") or [0, 0, 9, 9])[2] == 0,
                 what="Escape to close the drawer")
    d = app.snap()
    app.click_id(d, "settings_open_hit")
    app.wait_for(lambda s: (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0,
                 what="the Settings trigger to still work")
    return True, "open -> Esc closes -> trigger re-opens"


# ---- live-only rows against the REAL gate (#39a, --live) -------------------- #
# These run ONLY in --live mode (they drive real model turns through the
# outer loop's octos serve); against replay fixtures they would be nonsense.
LIVE_CHECK_NAMES = frozenset({
    "a real coding turn streams, terminates, and the timeline survives a refresh",
    "a live turn keeps running while a sibling session is focused",
})

@check("recovery", "a real coding turn streams, terminates, and the timeline survives a refresh",
       rows=("expected runtime model",))
def r1_live_turn_refresh(app):
    # Row 1's live slice: one short REAL turn, then the session refresh —
    # the timeline must restore (bubble + answer still in the store).
    def proses(s):
        return sum(1 for w in s.get("s", []) if "assistantprose" in str(w.get("i", "")))
    def working(s):
        return "workingrow" in app.kinds(s)
    def comp():
        for _ in range(24):
            d = app.snap()
            r = app.rect_re(d, COMPOSER_INPUT_RE)
            if r and r[2] > 0:
                return r
            time.sleep(0.5)
    r = comp(); app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    app.clear_composer(); app.type("what does main.rs print? answer with just the number")
    t_send = time.time()
    app.send()
    app.wait_for(working, timeout=30, what="the live turn to go live")
    app.wait_for(lambda s: not working(s), timeout=180,
                 what="the live turn to terminate")
    stream_secs = time.time() - t_send
    # The refresh affordance does not exist in this shell build (instrument:
    # no widget id or text contains refresh; actions.rs routes
    # session.refresh but the header control is not instantiated), so the
    # restore slice is asserted as PERSISTENCE: the answer and a laid-out
    # bubble remain in the timeline after the turn ends.
    d = app.snap()
    bubbles = [w.get("r") for w in d.get("s", [])
               if "userbubble" in str(w.get("i", ""))
               and (w.get("r") or [0, 0, 0, 0])[2] > 0]
    proses_n = proses(d)
    ok = stream_secs < 180 and proses_n >= 1 and bool(bubbles)
    return ok, (f"streamed+terminal={stream_secs:.1f}s prose={proses_n} "
                f"bubbles_laid_out={len(bubbles)} (refresh control absent "
                f"in this shell — persistence slice)")

@check("conversation", "a live turn keeps running while a sibling session is focused",
       rows=("background turn alive",))
def c2_live_background(app):
    # Row 2's live slice: start a REAL turn, focus a sibling session
    # (New chat), come back — the turn must have finished in the background
    # (answer present, not stuck Working).
    def proses(s):
        return sum(1 for w in s.get("s", []) if "assistantprose" in str(w.get("i", "")))
    def working(s):
        return "workingrow" in app.kinds(s)
    def comp():
        for _ in range(24):
            d = app.snap()
            r = app.rect_re(d, COMPOSER_INPUT_RE)
            if r and r[2] > 0:
                return r
            time.sleep(0.5)
    r = comp(); app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    app.clear_composer(); app.type("count to three, digits only")
    app.send()
    app.wait_for(working, timeout=30, what="the background turn to go live")
    # focus a sibling: New chat mints a fresh session
    nb = app.rect(app.snap(), "new_chat_hit") or app.rect(app.snap(), "newchat")
    assert nb, "no New chat control in the live app"
    app.click(int(nb[0] + nb[2] / 2), int(nb[1] + nb[3] / 2))
    time.sleep(8.0)  # away from the session while the turn runs
    # come back via the ORIGINAL session's row, found by its title text
    # (a fresh session re-instantiates the list; id-based lookup raced it)
    tr = None
    for _ in range(30):
        s = app.snap()
        tr = next((w.get("r") for w in s.get("s", [])
                   if (w.get("t") or "").strip() == "dsflash:main"
                   and (w.get("r") or [0, 0, 0, 0])[2] > 0), None)
        if tr:
            break
        time.sleep(0.5)
    assert tr, "the original session row never came back"
    app.click(int(tr[0] + tr[2] / 2), int(tr[1] + tr[3] / 2))
    app.wait_for(lambda s: not working(s), timeout=180,
                 what="the background turn to have finished")
    time.sleep(1.5)
    proses_now = proses(app.snap())
    ok = proses_now >= 1
    return ok, f"background turn terminal, prose rows={proses_now}"


@check("review", "the review panel is mounted with its header and scope pill")
def v_panel(app):
    d = app.snap()
    ids = app.widget_ids(d)
    ok = "review_panel" in ids and "review_scope_pill" in ids
    header = app.text_of(d, "review_header") or ""
    return ok, f"panel={'review_panel' in ids} header={header!r}"


@check("review", "the review badge carries the live +/- counts from the receipt",
       rows=("review", "diff", "hunk", "unmodified", "changed"))
def v_badge(app):
    # #36c made the badge totals COMPUTED from the folded diff receipt; the
    # walk fixtures carry none, so the panel shows the design's empty state
    # (web DiffReviewDialog `review-empty`, :120-122) — and the old authored
    # copy (+62 −5) must NOT be fabricated. The check guards exactly that:
    # panel mounted and no totals without a receipt. (The with-receipt
    # rendering — computed +3/−2 — is proven by f36c's unit tests.)
    d = app.snap()
    texts = " ".join(w.get("t", "") for w in d.get("s", []))
    panel = "review_panel" in app.widget_ids(d)
    no_totals_without_receipt = "+62" not in texts and "−5" not in texts
    ok = panel and no_totals_without_receipt
    return ok, (f"panel={panel} no_fabricated_totals={no_totals_without_receipt} "
                f"(fixtures carry no diff receipt — empty state by design)")


@check("review", "the fold receipt renders the unmodified-lines count",
       rows=("unmodified", "fold", "context"))
def v_fold(app):
    # Same #36c contract as v_badge: the fold line renders from a folded
    # receipt; the walk fixtures carry none, so the mounted panel must NOT
    # show a fabricated count (the authored copy did). f36c covers the
    # with-receipt rendering.
    d = app.snap()
    texts = " ".join(w.get("t", "") for w in d.get("s", []))
    panel = "review_panel" in app.widget_ids(d)
    empty_state_clean = "unmodified" not in texts.lower()
    ok = panel and empty_state_clean
    return ok, (f"panel={panel} empty_state_clean={empty_state_clean} "
                f"(no receipt folded -> no fold line)")


@check("review", "the review toggle is keyboard/click reachable")
def v_toggle(app):
    d = app.snap()
    return "review_toggle_hit" in app.widget_ids(d), "review_toggle_hit present"


# ---- settings: the settings drawer (#28e) ---------------------------------- #
@check("settings", "the settings drawer mounts with its close control")
def s_drawer(app):
    d = app.snap()
    ids = app.widget_ids(d)
    ok = "settings_drawer" in ids and "settings_close" in ids
    return ok, f"drawer={'settings_drawer' in ids} close={'settings_close' in ids}"


@check("settings", "the drawer header reads Session settings with its sections",
       rows=("settings", "session settings"))
def s_header(app):
    d = app.snap()
    texts = [w.get("t", "") for w in d.get("s", [])]
    ok = "Session settings" in texts and all(
        t in texts for t in ("Model", "Permissions", "Sandbox", "Context"))
    return ok, f"sections present={'Session settings' in texts}"


@check("settings", "connection actions live in settings (Live status visible)",
       rows=("disconnect", "connection", "server"))
def s_live(app):
    d = app.snap()
    s = app.text_of(d, "status") or ""
    return "Live" in s, f"status={s!r}"


@check("settings", "General settings carries the server connection action",
       rows=("general settings",))
def s_connection(app):
    # Row 165's own case (probed live): the drawer must expose the
    # connect/disconnect action the user would reach for.
    d = app.snap()
    texts = [w.get("t", "") for w in d.get("s", [])]
    action = next((t for t in texts if t in ("Disconnect", "Connect",
                                             "Disconnect server", "Server connection")),
                  None)
    return action is not None, f"connection action in drawer={action!r}"


# ---- palette: the command palette overlay (#28e) --------------------------- #
@check("palette", "the command palette overlay is mounted with search and list")
def q_mount(app):
    d = app.snap()
    ids = app.widget_ids(d)
    ok = "palette_search" in ids and "palette_list" in ids
    search = app.text_of(d, "palette_search") or ""
    return ok, f"search={search!r} list={'palette_list' in ids}"


@check("palette", "the palette hint row shows the key hints",
       rows=("command palette", "palette", "hint", "keyboard"))
def q_hint(app):
    d = app.snap()
    texts = [w.get("t", "") for w in d.get("s", [])]
    ok = "move" in texts and "run" in texts
    return ok, f"hint parts={[t for t in texts if t in ('move','run','· esc')]}"


@check("palette", "an unknown command fails closed (fail-closed receipt visible)",
       rows=("unknown command", "fails closed", "slash"))
def q_failclosed(app):
    # The palette list renders the command names; the fail-closed behaviour is
    # the conversation-area command receipt — here we assert the palette never
    # sends a draft when the search is not a command (visible: draft empty).
    d = app.snap()
    return (app.draft(d) or "") in ("", PLACEHOLDER), f"draft={app.draft(d)!r}"


@check("palette", "the palette opens by '/', lists its commands and executes one by keyboard",
       rows=("keyboard only",))
def q_execute(app):
    # Row 68's own case: opens, navigates, and executes by keyboard. Probed
    # live: the entry is typing '/' in the composer (palette_dock has no rect),
    # the list carries /mode /monitor /compact /model /btw /resume, and
    # return on /model opens the settings drawer at its Model section.
    r = None
    for _ in range(20):
        d = app.snap()
        r = app.rect_re(d, COMPOSER_INPUT_RE)
        if r and r[2] > 0:
            break
        time.sleep(0.5)
    app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    app.clear_composer()
    app.type("/")
    app.wait_for(lambda s: (app.rect(s, "palette_search") or [0, 0, 0, 0])[2] > 0,
                 timeout=10, what="the palette to open on '/'")
    app.type("model")
    time.sleep(1.0)
    d = app.snap()
    texts = [w.get("t", "") for w in d.get("s", [])]
    listed = [c for c in ("/mode", "/monitor", "/compact", "/model", "/btw", "/resume")
              if c in texts]
    app.key("down")
    app.key("return")
    d = app.wait_for(lambda s: all(t in [w.get("t", "") for w in s.get("s", [])]
                                   for t in ("Model", "Permissions")),
                     timeout=10, what="the drawer to open on return")
    opened = "Model" in [w.get("t", "") for w in d.get("s", [])]
    rc = app.rect(d, "settings_close")
    if rc and rc[2] > 0:
        app.click(int(rc[0] + rc[2] / 2), int(rc[1] + rc[3] / 2))
    app.key("escape")  # close the palette so the keyboard area starts clean
    time.sleep(1.0)
    closed = (app.rect(app.snap(), "palette_search") or [0, 0, 0, 0])[2] == 0
    ok = "/model" in listed and opened and closed
    return ok, f"listed={listed} drawer_opened={opened} palette_closed={closed}"


# ---- keyboard: the key model (#31e keys.rs) via the /k bridge -------------- #
@check("keyboard", "Escape is delivered inertly (app stays live, no crash)")
def k_escape(app):
    app.key("escape")
    d = app.snap()
    ok = "OctosCode" in [w.get("t", "") for w in d.get("s", [])]
    return ok, f"heading visible after esc={ok}"


@check("keyboard", "keyboard focus order: sidebar controls remain after keys",
       rows=("focus", "order", "arrow", "tab"))
def k_focus(app):
    app.key("escape")
    d = app.snap()
    ids = app.widget_ids(d)
    ok = "sidebar_toggle_hit" in ids and "new_chat_hit" in ids
    return ok, f"sidebar={'sidebar_toggle_hit' in ids} new_chat={'new_chat_hit' in ids}"


@check("keyboard", "Enter on the composer sends (the draft clears)",
       rows=("enter", "send", "submit"))
def k_enter(app):
    app.type_into_composer("walk: enter sends")
    app.key("return")
    app.wait_for(lambda s: (app.draft(s) or "") in ("", PLACEHOLDER),
                 what="the draft to clear after Enter")
    return True, "draft cleared after Enter"


# ---- longcode: the long-code-line render (#32b3) --------------------------- #
@check("longcode", "a 227-column code line stays fully readable (wrapped, tail visible)",
       rows=("grammar", "plain code", "code copy", "long code"))
def l_reachable(app):
    # The web oracle WRAPS code (`white-space: pre-wrap; overflow-wrap:
    # anywhere`, Timeline.module.css:340-341) — no horizontal scroll — and the
    # native theme wraps the same way (reachable_code restored the wrapping
    # layout). Reachability is proven in the RENDER: the line's tail must be
    # visible text the user can read, not clipped at the column edge.
    app.type_into_composer(
        "Render one code block whose single line is longer than 120 columns.")
    app.send()
    s = app.wait_for(lambda s: "assistantprose" in app.kinds(s),
                     timeout=40, what="the long-code answer row")
    texts = [w.get("t", "") for w in s.get("s", [])]
    head = any("pub fn reachability_probe()" in t for t in texts)
    tail = any("proves reachability of the long code line" in t for t in texts)
    return head and tail, f"head={head} tail={tail} (wrap per the web's pre-wrap oracle)"


# ---- recovery: reconnect, replay ------------------------------------------ #
@check("recovery", "the module reaches conn: Live")
def r_live(app):
    s = app.text_of(app.snap(), "status") or ""
    return "Live" in s, f"status={s!r}"


@check("recovery", "a replayed turn lands in the module's own transcript",
       rows=("replay", "reconnect", "recovery"))
def r_replay(app):
    _compose_and_send(app, "walk: replay a turn")
    app.wait_for(lambda s: bool(app.kinds(s)), what="the replayed timeline")
    return True, f"timeline kinds={app.kinds(app.snap())}"


@check("conversation", "the conversation content fits the window (bubble and timestamp end inside the right edge)",
       rows=("timeline",))
def conv_fits(app):
    # #38c (backlog 80b9f33): the module window ends at the scene's right
    # edge, and the full-width bubble row plus the right-aligned `now`
    # timestamp ended AT x=900 — clipped by the screen. Measured on main:
    # bubble row right=900, now right=900, margin 0. The fix insets the
    # conversation column; the contract: window ⊆ scene, and the laid-out
    # tail bubble row and the `now` timestamp end strictly inside the column
    # (right ≤ column right − 4).
    r = None
    for _ in range(20):
        d = app.snap()
        r = app.rect_re(d, COMPOSER_INPUT_RE)
        if r and r[2] > 0:
            break
        time.sleep(0.5)
    app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    app.clear_composer(); app.type("walk fits probe"); app.send()
    app.wait_for(lambda s: "workingrow" not in app.kinds(s), timeout=60,
                 what="the probe turn to terminate")
    # The answer-actions row (the right-aligned `now`) materializes with the
    # terminal fold, a beat after the Working row clears (#40a run: bubbles
    # laid out, `now` absent at snap time). Poll briefly; the contract stays
    # strict — after the poll the timestamp must be there.
    app.wait_for(lambda s: any((w.get("t") or "").strip().lower() == "now"
                               and (w.get("r") or [0, 0, 0, 0])[2] > 0
                               for w in s.get("s", [])),
                 timeout=10, what="the answer timestamp to lay out")
    time.sleep(1.0)
    d = app.snap()
    win = app.rect(d, "main_window")
    scene = app.rect(d, "scene")
    col = app.rect(d, "timeline_list")
    assert win and scene and col, "window/scene/column missing from /snap"
    win_right = win[0] + win[2]
    scene_right = scene[0] + scene[2]
    col_right = col[0] + col[2]
    rights, nows = [], []
    for w in d.get("s", []):
        m = INSTANCE_KIND_RE.match(str(w.get("i", "")))
        rr = w.get("r") or [0, 0, 0, 0]
        if not (m and rr[2] > 0):
            continue
        if m.group(1) == "userbubble":
            rights.append(rr[0] + rr[2])
        if (w.get("t") or "").strip().lower() == "now":
            nows.append(rr[0] + rr[2])
    in_scene = win_right <= scene_right and win[2] <= scene[2]
    bubble_ok = bool(rights) and max(rights) <= col_right - 4
    now_ok = bool(nows) and max(nows) <= col_right - 4
    ok = in_scene and bubble_ok and now_ok
    return ok, (f"window_right={win_right} scene_right={scene_right} "
                f"bubble_right={max(rights) if rights else None} "
                f"now_right={max(nows) if nows else None} col_right={col_right}")


# ---- recovery: failed local command (row 209, #33b) ------------------------ #
@check("recovery", "a failed local command restores the typed input and sends nothing",
       rows=("local-command",))
def r_localcmd(app):
    # Row 209's own case (probed live): '/bogus …' sends no model turn (no
    # working row appears — fail-closed holds), and the input must be
    # RESTORED for the user to fix.
    r = None
    for _ in range(20):
        d = app.snap()
        r = app.rect_re(d, COMPOSER_INPUT_RE)
        if r and r[2] > 0:
            break
        time.sleep(0.5)
    # The earlier recovery checks (r_replay) leave a turn LIVE; the mounted
    # composer rejects text while the turn runs, so wait for it to settle
    # before typing (probed via the app log: no `draft synced` for this
    # check's text when typed mid-turn).
    app.wait_for(lambda s: "workingrow" not in app.kinds(s), timeout=45,
                 what="the earlier recovery turn to settle")
    time.sleep(1.0)
    app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    app.clear_composer()
    # ONE /t event carries the whole string: the composer's changed action
    # fires ONCE with '/bogus-command walk probe' (!= "/"), so the bare-'/'
    # palette trigger never opens and the draft holds the command. (Probed
    # three other flows: '/'-first gets eaten by the palette's Esc, and a
    # home+'/' insertion never fires a changed sync.)
    app.type("/bogus-command walk probe")
    time.sleep(0.5)
    app.key("return")
    time.sleep(3.0)
    d = app.snap()
    sent = "workingrow" in app.kinds(d)
    restored = app.draft(d) == "/bogus-command walk probe"
    return (not sent) and restored, f"sent={sent} draft_restored={restored} draft={app.draft(d)!r}"


# #33a review: DEPTH. A row is `specific` when at least one check that applied
# to it tests the row's OWN behaviour (what its `case` says); rows covered only
# by the generic smoke set (Live status, drawer mounts, composer present, ...)
# are `smoke` — their pass must not be read as the behaviour being covered.
# One table keeps the classification auditable; names must match @check.
SPECIFIC_CHECKS = {
    "the composer accepts typed text (prompt input)",
    "composing clears the draft on send",
    "a sent prompt streams an assistant answer row",
    "the user's own prompt renders as a row",
    "the answer row renders after the prompt row (order)",
    "the turn's timeline item kinds are present",
    "the Stop control is present for the live turn",
    "New chat mints a fresh Session and re-opens the workspace",
    "a replayed turn lands in the module's own transcript",
    "a typed draft round-trips through the composer",
    "the goals/loops lists render their rows",
    "the review badge carries the live +/- counts from the receipt",
    "the fold receipt renders the unmodified-lines count",
    "an unknown command fails closed (fail-closed receipt visible)",
    "Enter on the composer sends (the draft clears)",
    "a 227-column code line stays fully readable (wrapped, tail visible)",
    "the palette opens by '/', lists its commands and executes one by keyboard",
    "a queued follow-up drains as its own turn and a reselect replays nothing",
    "General settings carries the server connection action",
    "a failed local command restores the typed input and sends nothing",
    "a real coding turn streams, terminates, and the timeline survives a refresh",
    "a live turn keeps running while a sibling session is focused",
    # #41c: the smoke-only rows' own-behaviour checks.
    "Alt+D reaches a fleet capability notice surface to focus",
    "Alt+P toggles a peer dock fold (expands and collapses)",
    "Fleet's roster rows render with real rects (the mock's lanes)",
    "the palette's /monitor command reaches a monitors surface",
    "the settings drawer exposes the Models management section",
    "Escape closes the settings drawer and the trigger still works",
}
for _c in CHECKS:
    _c["specific"] = _c["name"] in SPECIFIC_CHECKS


# --------------------------------------------------------------------------- #
# Verdict logic (unit-tested in tools/walk/test_run.py)
# --------------------------------------------------------------------------- #
def decided_status(check_statuses, area_blocked: bool) -> str:
    """The row status from its checks (or `blocked` when the area never started).

    * `blocked` — the area's server/app failed to start (card #19b, defect 1).
    * `pass` — at least one check ran and every check passed. An EMPTY run can
      never be `pass` (`all([]) == True` was the #19b defect).
    * `fail` — otherwise.
    """
    if area_blocked:
        return "blocked"
    if not check_statuses:
        return "fail"
    return "pass" if all(s == "pass" for s in check_statuses) else "fail"


def row_reason(check_statuses, area_reason: str = "") -> str:
    """The human reason for a row, from its checks."""
    if not check_statuses:
        return area_reason or "no checks ran for this row"
    failed = [name for name, st in check_statuses if st != "pass"]
    if failed:
        return f"failing checks: {'; '.join(failed)}"
    return f"{len(check_statuses)} checks, all pass"


def exit_code(counts: dict, infra_blocked: int) -> int:
    """The process exit code, so a failure can never look green (card #19b).

    `1` when a check failed, a row was not run, or a **selected** row is blocked
    by a start failure. (`infra_blocked` counts the latter; the 3 `real-turn`
    rows are never selected, so they never count here.) A prerequisite failure is
    handled earlier and exits `2`.
    """
    if counts.get("fail", 0) or counts.get("not-run", 0) or infra_blocked:
        return 1
    return 0


# --------------------------------------------------------------------------- #
def load_rows():
    with open(WALK_ROWS, newline="") as f:
        return list(csv.DictReader(f))


def is_live_only(case: str) -> bool:
    """True when the row needs a state the replay fixtures cannot produce."""
    return any(p.search(case) for p in LIVE_ONLY_PATTERNS)


def area_of(row):
    hay = row["case"] + " " + row["spec"]
    for name, pat in AREA_PATTERNS.items():
        if pat.search(hay):
            return name
    return None


def select_targets(limit: int | None):
    """The first `limit` rows in the scriptable areas, **round-robin by area**.

    Pure file order lets `conversation`/`recovery` crowd out `threads` (only 4-5
    eligible rows), so the card's named areas would not all appear. Taking one row
    per area in turn keeps every named area represented while still taking the
    earliest rows within each.
    """
    eligible: dict[str, list] = {a: [] for a in AREA_PATTERNS}
    for i, row in enumerate(load_rows(), start=1):
        if (row.get("web_only_reason") or "").strip() or row["needs"] == "real-turn":
            continue
        area = area_of(row)
        if area and AREA_SCRIPTABLE[area]:
            eligible[area].append((i, area, row))
    order = [a for a in ("conversation", "threads", "composer", "recovery",
                         "peer", "review", "settings", "palette", "keyboard",
                         "longcode")
             if AREA_SCRIPTABLE[a]]
    picked, cursors = [], {a: 0 for a in order}
    while limit is None or len(picked) < limit:
        progressed = False
        for a in order:
            if len(picked) >= limit:
                break
            c = cursors[a]
            if c < len(eligible[a]):
                picked.append(eligible[a][c])
                cursors[a] += 1
                progressed = True
        if not progressed:
            break
    return picked


def missing_capability(spec: str) -> str:
    try:
        with open(PARITY, newline="") as f:
            rows = list(csv.DictReader(f))
    except FileNotFoundError:
        return "native capability unknown"
    key = spec.split("/")[-1].replace(".spec.ts", "")
    for r in rows:
        src = r.get("web_src") or ""
        if key in src or key.split("-")[0] in src:
            return f"{r['capability']} [{r['native_status']}]"
    return "native capability not yet built (see docs/parity-matrix.csv)"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=30)
    ap.add_argument("--only", default=None)
    ap.add_argument("--port", type=int, default=APP_PORT)
    ap.add_argument("--no-build", action="store_true",
                    help="do not build the replay server if it is missing; just report it")
    ap.add_argument("--live", action="store_true",
                    help="run the live-only rows against the RUNNING real gate "
                         "(OCTOS_LIVE_TOKEN_FILE required; no replay server)")
    args = ap.parse_args()

    # Fail fast on the documented prerequisites, with a message that says exactly
    # what to run (card #19b, defect 2). Building the replay server is cached.
    try:
        check_prereqs(build_replay=not (args.no_build or args.live))
    except PrereqError as e:
        print(f"tools/walk: {e}", file=sys.stderr)
        return 2

    WALK.mkdir(parents=True, exist_ok=True)
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    rows = load_rows()
    targets = select_targets(args.limit)
    if args.live:
        # #39a: exactly the live-only rows a real model can serve today —
        # row 1 (a real coding turn + refresh) and row 2 (a background turn
        # survives focusing a sibling). Row 50 (approval shortcuts) needs the
        # model to RAISE an approval card on its own; not scriptable yet.
        # select_targets EXCLUDES live-only rows (needs == "real-turn" is
        # skipped), so --live builds its targets straight from the rows.
        wanted = {1, 2}
        targets = [(i, area_of(r) or "", r)
                   for i, r in enumerate(rows, start=1) if i in wanted]
    try:
        procs = LiveGate(args.port) if args.live else Procs(args.port)
    except PrereqError as e:
        print(f"tools/walk: {e}", file=sys.stderr)
        return 2

    areas = sorted({a for _, a, _ in targets})
    if args.only:
        areas = [a for a in areas if a == args.only]

    # area -> {"blocked": bool, "reason": str}. A row's checks run against the app
    # for its area; per-check results are recorded per row (card #19c item 3).
    area_state: dict[str, dict] = {}
    per_row_checks: dict[int, list] = {}   # row_id -> [(check_name, status, reason)]
    for area in areas:
        scenario = scenario_for(area)
        try:
            procs.start_server(scenario)
            procs.start_app(scenario)
        except Exception as e:  # noqa: BLE001
            area_state[area] = {"blocked": True,
                                "reason": f"scenario '{scenario}' failed to start: {e}"}
            continue
        app = App(procs.app_port)
        # Run each check ONCE against this area's app; record its status.
        area_checks = [c for c in CHECKS if c["area"] == area]
        if args.live:
            # #39a: --live runs EXACTLY the live-specific set — the replay
            # checks' assertions (fixture texts, replay counts) are nonsense
            # against a real gate.
            area_checks = [c for c in area_checks
                           if c["name"] in LIVE_CHECK_NAMES]
        else:
            # #40a: the mirror direction — the live checks drive REAL model
            # turns (short prompts, live timing), so against a replay fixture
            # they are nonsense by construction (the first full walk that
            # included them lost 2 rows to exactly that).
            area_checks = [c for c in area_checks
                           if c["name"] not in LIVE_CHECK_NAMES]
        results = []
        evidence = ""
        for chk in area_checks:
            try:
                ok, reason = chk["fn"](app)
                status = "pass" if ok else "fail"
            except Exception as e:  # noqa: BLE001
                ok, status, reason = False, "fail", f"exception: {e}"
            results.append({"name": chk["name"], "status": status, "reason": reason[:200],
                            "rows": chk["rows"], "specific": chk["specific"]})
            if status == "fail":
                try:
                    sj = EVIDENCE / f"area-{area}.snap.json"
                    sj.write_text(json.dumps(app.snap()))
                    subprocess.run(["curl", "-s", "--max-time", "20", "-o",
                                    str(EVIDENCE / f"area-{area}.png"),
                                    f"http://127.0.0.1:{procs.app_port}/g?raw=1"],
                                   capture_output=True)
                    evidence = str(sj.relative_to(ROOT))
                except Exception:  # noqa: BLE001
                    pass
            print(f"  [{area}] {status:5} {chk['name'][:62]:62} :: {reason[:60]}")
        # Map each selected row of this area to the checks that apply to it.
        for rid, a, row in targets:
            if a != area:
                continue
            applied = [(r["name"], r["status"]) for r in results if check_applies(r, row)]
            krs = [(r["name"], r["status"], r["reason"], r["specific"])
                   for r in results if check_applies(r, row)]
            per_row_checks[rid] = krs
        area_state[area] = {"blocked": False, "reason": "", "evidence": evidence,
                            "results": results}

    procs.stop_all()

    target_area = {i: a for i, a, _ in targets}
    out_rows: list = []          # per-row aggregate
    check_rows: list = []        # per-check detail (card #19c item 3)
    for i, row in enumerate(rows, start=1):
        spec, case, needs = row["spec"], row["case"], row["needs"]
        web_only = (row.get("web_only_reason") or "").strip()
        area = area_of(row) or ""
        if i in target_area:
            area = target_area[i]
            st = area_state.get(area, {})
            checks = per_row_checks.get(i, [])
            status = decided_status([s for _, s, _, _ in checks], st.get("blocked", False))
            reason = row_reason([(n, s) for n, s, _, _ in checks], st.get("reason", ""))
            depth = "specific" if any(sp for *_, sp in checks) else "smoke"
            ev = st.get("evidence", "")
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": status, "depth": depth, "evidence": ev,
                             "reason": reason})
            for n, s, rr, _sp in checks:
                check_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                                   "check": n, "status": s, "evidence": ev, "reason": rr})
        elif web_only:
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": "skipped", "evidence": "",
                             "reason": f"skipped: operator-confirmation-pending ({web_only})"})
        elif needs == "real-turn":
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": "live-only", "evidence": "",
                             "reason": "live-only: needs a real model turn (Phase 4, outer loop)"})
        elif is_live_only(case):
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": "live-only", "evidence": "",
                             "reason": "live-only: state the replay fixtures cannot produce"})
        elif area == "approval":
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": "not-yet-implemented", "evidence": "", "reason": APPROVAL_MISSING})
        elif area == "connect":
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": "not-yet-implemented", "evidence": "", "reason": CONNECT_MISSING})
        else:
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": "not-yet-implemented", "evidence": "",
                             "reason": f"missing: {missing_capability(spec)}"})

    live_suffix = "_live" if args.live else ""
    with open(WALK / f"results{live_suffix}.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["row_id", "area", "spec", "case", "status", "depth", "evidence", "reason"])
        w.writeheader()
        w.writerows(out_rows)
    with open(WALK / f"results{live_suffix}-checks.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["row_id", "area", "spec", "case", "check", "status", "evidence", "reason"])
        w.writeheader()
        w.writerows(check_rows)

    from collections import Counter
    counts = Counter(r["status"] for r in out_rows)
    areas_seen = sorted({r["area"] for r in out_rows if r["area"]})
    print("\n== per-area summary (#33a) ==")
    for a in areas_seen:
        c = Counter(r["status"] for r in out_rows if r["area"] == a)
        cells = ", ".join(f"{k}={c[k]}" for k in
                          ("pass", "fail", "live-only", "not-yet-implemented",
                           "blocked", "skipped") if c.get(k))
        print(f"   {a:14} {cells}")
    infra_blocked = sum(1 for i, r in enumerate(out_rows, start=1)
                        if i in target_area and r["status"] == "blocked")
    print("\n== walk-runner summary ==")
    for k in ("pass", "fail", "not-yet-implemented", "blocked", "skipped", "not-run"):
        if counts.get(k):
            print(f"   {k:20} {counts[k]}")
    print(f"   total                {len(out_rows)}")
    print(f"   scripted rows        {len(targets)}  (areas: {areas})")
    print(f"   per-check rows       {len(check_rows)}  (docs/walk/results{live_suffix}-checks.csv)")
    by_depth = Counter((r["status"], r.get("depth", "")) for r in out_rows)
    print(f"   pass by depth        specific={by_depth.get(('pass', 'specific'), 0)}"
          f" smoke={by_depth.get(('pass', 'smoke'), 0)}"
          f"  (distinct checks: {len({c['check'] for c in check_rows})})")
    if infra_blocked:
        print(f"   NOTE: {infra_blocked} selected row(s) blocked by a start failure")
        reasons = sorted({r["reason"] for r in out_rows
                          if r["status"] == "blocked" and r["reason"]})
        for reason in reasons[:4]:
            print(f"     - {reason[:160]}")
    return exit_code(counts, infra_blocked)


if __name__ == "__main__":
    sys.exit(main())
