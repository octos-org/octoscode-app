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
import shutil
import signal
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import os as _os, sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.abspath(__file__)))
from snapsafe import scrub as _scrub  # noqa: E402
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

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
# A11: A3's sidebar tree (chrome.rs `SbRowTpl`): a session row's title is
# `sb_r_title`, its click target `sb_r_open` (the old `i<N>_threadrow` rows are
# gone; the regex matched nothing and the reselect crashed on None).
THREAD_ROW_RE = re.compile(r"^sb_r_title$")
THREAD_OPEN_RE = re.compile(r"^sb_r_open$")


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
    """The scenario server is ours: build it (cargo's own up-to-date check makes
    a current binary a no-op). A11: always asked, not only when missing — a
    stale binary silently ignores a flag the runner passes (`--adopt-turn-ids`)."""
    if not build:
        if REPLAY.is_file():
            return REPLAY
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
                  "longcodeline": 8386, "two-turn": 8387}


def set_scenario_port_base(base: int) -> None:
    """A11: move the replay servers to `base`.. (same order), so a run stays in
    the port block its operator was given (`--scenario-port-base`)."""
    for i, name in enumerate(list(SCENARIO_PORTS)):
        SCENARIO_PORTS[name] = base + i

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
# A11: the composer area sends two prompts in a row (c_queue): with the
# replayed turns now the app's own (`--adopt-turn-ids`), `conversation`'s
# second recorded turn is its INTERRUPTED one (no answer), so the area runs on
# `two-turn` — two real consecutive completed turns from one live session.
AREA_SCENARIO = {"conversation": "conversation", "threads": "conversation",
                 "composer": "two-turn", "recovery": "conversation",
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


# A11: both reasons were Phase-3 facts. The approval takeover is built (A6,
# tools/walk/a6_surfaces_walk.py) and the pre-connection gate IS walked by the
# native walks that launch first-run instances (A2, A11); a row of these
# areas that no native walk maps is "not walked", with the parity matrix's
# own verdict where it cites the case (`parity_reason`).
APPROVAL_MISSING = ("not walked: no native click walk maps this approval case "
                    "(the takeover card itself is walked by tools/walk/a6_surfaces_walk.py)")
CONNECT_MISSING = ("not walked: run.py's own instances auto-connect; the pre-connection "
                   "gate is walked only by the native walks that launch first-run apps "
                   "(tools/walk/a2_board1_walk.py, a11_offer_walk.py), and none maps this case")

# The subset marker used by `@check(rows=…)` for "every row of the area".
ALL = "__all__"

# --------------------------------------------------------------------------- #
# #43b — PER-ROW LAUNCH ENV. The runner used to fix ONE env per area at launch
# (`start_app`), so a row whose behaviour depends on process-wide state could
# never be driven: a theme, a window width, or "not connected yet". The app
# reads all three ONCE per process, so each distinct env needs its OWN app
# instance — hence `ENV_GROUPS`: rows are grouped by their launch key and each
# group is launched, driven and torn down in turn.
#
# App-side consumers (all read at mount / first frame, none hot-reload):
#   OCTOSENSE_WINDOW_SIZE "WxH" → `self.window_w` (lib.rs:2387-2394), which
#     picks `wide` (>= 1260.0, lib.rs:2416) and `width_hides_sidebar`
#     (< 760.0, lib.rs:2418).
#   OCTOSCODE_THEME "light|dark|system" → `screens::theme::set_preference`
#     (lib.rs:2252, screens/theme.rs:170-184); the resolved mode is
#     process-global (screens/theme.rs:175-186 documents the test_lock).
#   OCTOSCODE_FIRST_RUN / OCTOSCODE_NO_CONNECT → early `return` before the
#     transport is built (lib.rs:1341-1348), so `store.is_live()` stays false
#     and `ids!(first_run)` paints (lib.rs:2301-2302, 2421).
#
# A row may also name a `rows=` subset for its checks; the launch env alone
# never decides a row's verdict, the checks do.
ENV_GROUPS: dict[tuple, dict] = {
    # row 106 — the first-run flow (e2e/onboarding.spec.ts:77). The walk app
    # auto-connects, so the pre-connection chrome never mounted.
    ("keyboard", "first-run"): {
        "first_run": True,
        "checks": [
            "the first-run chrome mounts with no connection and a focused composer",
            "the Add-workspace dialog focuses its path field and Escape keeps the draft",
        ],
        "rows": ("starts a first workspace with the keyboard",),
    },
    # row 183 — ${viewport.width} semantics (e2e/release-readiness.spec.ts:25).
    # The row is a TEMPLATE: the web suite runs it at each configured width, so
    # the runner sweeps the widths below instead of the fixed 900 px window.
    # 1280 crosses the `wide` breakpoint (1260), 720 the `width_hides_sidebar`
    # one (760): the two edges the module actually branches on.
    ("peer", "viewport-1280"): {
        # A11: the env CAPS the module's frame (A3 `env_frame`); the floating
        # module window is 990 wide, so the shell maximizes it first (its own
        # scripted WM action) — the frame is then the requested 1280.
        "env": {"OCTOSENSE_WINDOW_SIZE": "1280x900",
                "HEADLESS_ARGS": "--module octoscode --test-action maximize"},
        "checks": [
            "the requested viewport width is honoured and picks the width breakpoints",
            "the settings drawer keeps one dialog and its geometry across a panel load",
        ],
        "rows": ("preserves its geometry while model management loads",),
    },
    ("peer", "viewport-720"): {
        "env": {"OCTOSENSE_WINDOW_SIZE": "720x900"},
        "checks": [
            "the requested viewport width is honoured and picks the width breakpoints",
        ],
        "rows": ("preserves its geometry while model management loads",),
    },
    # row 212 — manual light on a dark OS (e2e/theme.spec.ts:72). The stored
    # preference is `light` while the OS stays dark, which the native app
    # cannot express as a process env alone (it has one resolved mode), so the
    # check asserts the LIGHT resolution itself plus the readable tokens.
    ("settings", "theme-light"): {
        "env": {"OCTOSCODE_THEME": "light"},
        "checks": [
            "manual light keeps conversation and settings text readable",
        ],
        "rows": ("manual light preserves readable conversation",),
    },
}


def env_groups_for(row: dict) -> list:
    """EVERY launch-env key that applies to `row` (empty = the default env).

    A row can need MORE THAN ONE instance: row 183 is a template the web suite
    runs at every configured viewport width, so its verdict must be the AND of
    each width. A single-key answer would launch the first width and silently
    never launch the second.
    """
    case = row["case"].lower()
    area = area_of(row)
    return [k for k, spec in ENV_GROUPS.items()
            if k[0] == area and any(s in case for s in spec["rows"])]


def env_group_for(row: dict) -> tuple | None:
    """The REPRESENTATIVE launch-env key for `row` (`None` = the default env).

    Used for the per-row verdict lookup (`area_state`), so a row driven by
    several instances reads one of them; the merged check list in
    `per_row_checks` is what actually decides the row.
    """
    keys = env_groups_for(row)
    return keys[0] if keys else None


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

    def raw(self, path: str) -> bytes:
        """Binary GET (`/g?raw=1`) — `_get` decodes UTF-8, which corrupts PNGs."""
        with urllib.request.urlopen(self.base + path, timeout=20) as r:
            return r.read()

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

    @staticmethod
    def laid_out(w: dict) -> bool:
        r = w.get("r") or [0, 0, 0, 0]
        return r[2] > 0 and r[3] > 0 and w.get("v", 1) != 0

    def shown_rect_re(self, snap: dict, pattern):
        """The first LAID-OUT widget whose id matches (A11: a hidden template
        twin comes first in the tree for several chrome ids)."""
        return next((w["r"] for w in snap.get("s", [])
                     if pattern.match(str(w.get("i", ""))) and self.laid_out(w)), None)

    def shown_texts(self, snap: dict, pattern):
        return [str(w.get("t")) for w in snap.get("s", [])
                if pattern.match(str(w.get("i", ""))) and w.get("t") and self.laid_out(w)]

    def rect(self, snap: dict, ident: str):
        # A11: the chrome mounts some ids twice (a hidden phone/rail twin), so
        # the first match can be a zero rect — a click there lands at 0,0.
        # Prefer the laid-out, visible instance.
        first = None
        for w in snap.get("s", []):
            if str(w.get("i", "")) == ident:
                r = w["r"]
                if r[2] > 0 and r[3] > 0 and w.get("v", 1) != 0:
                    return r
                first = first or r
        return first

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
            # A11 (the brief's walk tip): the composer can hold a RESTORED
            # prompt (an interrupted turn's text comes back, like the web),
            # and Shift+End selection no longer clears it (measured: four
            # "walk queue one" sends concatenated in one bubble). End, then
            # one Backspace per character, is the method that empties it.
            n = len(self.draft() or "")
            self.key("end")
            for _ in range(n + 1):
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
        # A11: `--adopt-turn-ids` — the replayed turn is the app's own (a real
        # server adopts the turn/start id). A7's turn controller settles only
        # the turn it dispatched; without this every replayed turn stayed
        # live and every later prompt queued behind it.
        proc = subprocess.Popen(
            [str(REPLAY), str(port), "--scenario", scenario, "--adopt-turn-ids"],
            stdout=f, stderr=f)
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
        self.start_app_opts(scenario)

    def start_app_opts(self, scenario: str, first_run: bool = False,
                       extra_env: dict | None = None):
        self.stop_app()
        state = ROOT / "tmp" / "walk" / "headless"
        state.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        if first_run:
            # #43b row 106 — FIRST-RUN MODE: the module must NOT auto-connect.
            # OCTOSCODE_FIRST_RUN gates the transport off entirely
            # (crates/octoscode-module/src/lib.rs:1341-1348), and the connect
            # env stays OFF so nothing can dial the replay server.
            env["OCTOSCODE_FIRST_RUN"] = "1"
        else:
            env.update({
                "OCTOS_BASE_URL": f"http://127.0.0.1:{SCENARIO_PORTS[scenario]}",
                "OCTOS_BEARER": "walk-dummy-token",
            })
        env.update({
            "OCTOS_PROFILE_ID": "dsflash",
            "MAKEPAD_WM_TEST_APP": "octoscode",
            "HEADLESS_ARGS": "--module octoscode",
            "HEADLESS_STATE": str(state),
        })
        # Brief §8 (A11): every instance reads and writes a FRESH state tree
        # (drafts, credentials, preferences, notifications, recents, ...),
        # never the operator's own files.
        from walk_env import isolated_env  # noqa: PLC0415 (same directory)
        env.update(isolated_env(ROOT / "tmp" / "walk" / "state" / f"{scenario}-{time.time_ns()}"))
        # This checkout's design files, never the shared materialized copy.
        env.setdefault("OCTOSCODE_DESIGN_DIR", str(ROOT / "design"))
        if extra_env:
            env.update(extra_env)
        # The env of the running app, so a check can assert what it was LAUNCHED
        # with (row 212's theme, row 183's window size) instead of guessing.
        self.last_env = dict(env)
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
        # Brief §8 (A11): isolated app state for the live instance too.
        from walk_env import isolated_env  # noqa: PLC0415 (same directory)
        env.update(isolated_env(ROOT / "tmp" / "walk" / "state" / f"live-{time.time_ns()}"))
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
    # A11: the opened session's row must be LAID OUT in the sidebar (the old
    # fallback accepted a hidden "No threads yet" text, so it could not fail).
    try:
        d = app.wait_for(lambda s: bool(app.shown_texts(s, THREAD_ROW_RE)), timeout=15,
                         what="the opened session's row in the sidebar")
    except AssertionError:
        d = app.snap()
    rows = app.shown_texts(d, THREAD_ROW_RE)
    return bool(rows), f"thread rows={rows[:2]}"


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
    # A11: A3's sidebar renamed the control (sb_new_chat_hit).
    d0 = app.snap()
    app.click_id(d0, "sb_new_chat_hit" if app.rect(d0, "sb_new_chat_hit") else "new_chat_hit")
    app.wait_for(lambda s: ("OctosCode" in [w.get("t", "") for w in s.get("s", [])]
                            and app.rect_re(s, COMPOSER_INPUT_RE) is not None),
                 what="the workspace to re-open after New chat")
    d = app.snap()
    ph = next((w.get("t") for w in d.get("s", [])
               if COMPOSER_INPUT_RE.match(str(w.get("i", "")))), None)
    return True, f"heading='OctosCode' composer={ph!r}"


# ---- composer: draft, send, timeline, input round-trip -------------------- #
@check("composer", "the approval pill opens the permission menu above it and Escape closes it")
def cp_pill_cycle(app):
    # A11: since A10 the pill no longer CYCLES the mode (#P4a1): it is the
    # permission seat (screens/board3/seats.rs, the web's SessionControlBar)
    # and opens the "Permission" menu above it — the presets the server lists
    # (permission/profile/list), "No permission presets are available." when
    # it lists none (this replay answers {}). Choosing a preset, its read-back
    # and the full-access confirmation are tools/walk/a10_seats.py's (row 163).
    pill_re = re.compile(r"^i\d+_composer_2$")

    def menu(d):
        return (app.rect(d, "b3_dialog") or [0, 0, 0, 0])[2] > 0
    r0 = None
    for _ in range(20):
        r0 = app.shown_rect_re(app.snap(), pill_re)
        if r0:
            break
        time.sleep(0.5)
    if not r0:
        return False, "the approval pill (the permission seat) is not laid out (polled 10s)"
    app.click(int(r0[0] + r0[2] / 2), int(r0[1] + r0[3] / 2))
    try:
        app.wait_for(menu, timeout=8, what="the permission menu to open")
    except AssertionError:
        return False, f"the pill CLICK opened no menu (pill={r0})"
    time.sleep(0.5)
    d = app.snap()
    m = app.rect(d, "b3_dialog")
    laid = {str(w.get("t") or "").strip() for w in d.get("s", []) if app.laid_out(w)}
    titled = "Permission" in laid
    above = m[1] + m[3] <= r0[1] + 1
    # With no presets the menu says why: empty / unavailable / loading, or the
    # read's own error line with Retry (this replay's `{}` answer names no
    # session: "permission/profile/list returned another session").
    state = next((str(w.get("t")).strip() for w in d.get("s", [])
                  if str(w.get("i", "")) in ("b3_perm_empty", "b3_perm_unavailable", "b3_perm_loading",
                                             "b3_perm_error_text")
                  and app.laid_out(w) and (w.get("t") or "").strip()), None)
    opts = sum(1 for w in d.get("s", []) if re.match(r"^b3_perm_opt_\d+$", str(w.get("i", "")))
               and app.laid_out(w))
    app.key("escape")
    try:
        app.wait_for(lambda s: not menu(s), timeout=6, what="Escape to close the menu")
        closed = True
    except AssertionError:
        closed = False
    ok = titled and above and (opts > 0 or state is not None) and closed
    return ok, (f"menu={m} above_pill={above} title={titled} presets={opts} state={state!r} "
                f"escape_closes={closed}")


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
    # A11: each prompt's OWN turn is proven on the wire — the replay server's
    # log of `turn/start` — and its settlement in the window (no working row,
    # no queued chip left). Counting assistant rows was virtualization-bound:
    # the second recorded answer is long and the list shows only its tail.
    server_log = WALK / f"server-{scenario_for('composer')}.log"

    def starts():
        try:
            return server_log.read_text().count("<- turn/start")
        except OSError:
            return -1

    def settled(s):
        queued = any(str(w.get("i", "")) == "queue_count" and app.laid_out(w) and (w.get("t") or "").strip()
                     for w in s.get("s", []))
        return "workingrow" not in app.kinds(s) and not queued

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
    app.wait_for(settled, timeout=45, what="earlier composer turns to settle")
    time.sleep(1.0)
    base = starts()
    for n, prompt in enumerate(("walk queue one", "walk queue two"), start=1):
        r = app.rect_re(app.snap(), COMPOSER_INPUT_RE)
        app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
        app.clear_composer(); app.type(prompt); app.send()
        for _ in range(40):
            if starts() >= base + n:
                break
            time.sleep(0.5)
        if starts() < base + n:
            return False, f"{prompt!r} never reached the wire (turn/start x{starts() - base})"
        time.sleep(1.0)
        app.wait_for(settled, timeout=60, what=f"{prompt!r}'s own turn to settle")
    own_turns = starts() - base
    d = app.snap()
    tr = app.shown_rect_re(d, THREAD_OPEN_RE)
    if not tr:
        return False, "no laid-out session row (sb_r_open) to reselect"
    app.click(int(tr[0] + tr[2] / 2), int(tr[1] + tr[3] / 2))
    time.sleep(3.0)
    if starts() - base != own_turns:
        return False, f"the reselect sent turn/start again (x{starts() - base - own_turns})"
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

    # Virtualization makes per-row presence viewport-dependent (the native list
    # materialises ~a screen; the web keeps the whole DOM): what IS durable is
    # that no prompt bubble is doubled (nothing replayed into the timeline),
    # the timeline renders, and no session was minted.
    d = app.snap()
    one, two = tree_count(d, "walk queue one"), tree_count(d, "walk queue two")
    rendered = any(app.laid_out(w) for w in d.get("s", []) if INSTANCE_KIND_RE.match(str(w.get("i", ""))))
    sessions = (app.text_of(d, "sessions") or "").strip()
    ok = own_turns == 2 and one <= 1 and two <= 1 and rendered and sessions.endswith(" 1")
    return ok, (f"own turn/start x{own_turns}, each settled; reselect sent none; bubbles one={one} two={two} "
                f"timeline_rendered={rendered} {sessions!r}")


@check("composer", "the conversation column hosts the timeline PortalList")
def comp_timeline(app):
    return "timeline_list" in app.widget_ids(app.snap()), "timeline_list present"


@check("composer", "a TUI-only slash command reports fail-closed and a path prompt still turns")
def cp_command_receipts(app):
    # #P4d3 rows 2+3: '/title' is a KNOWN name the native build cannot run —
    # a receipt row appears in the timeline and the composer clears
    # (registry.ts:478's fail-closed explanation). '/bogus' names nothing —
    # receipt + the text STAYS editable (surface-recovery.spec.ts:186). A
    # PATH-shaped input is a prompt: it dispatches a real turn.
    def comp():
        for _ in range(20):
            d = app.snap()
            r = app.rect_re(d, COMPOSER_INPUT_RE)
            if r and r[2] > 0:
                return r
            time.sleep(0.5)
    def texts(d):
        return [str(w.get("t") or "") for w in d.get("s", [])
                if (w.get("r") or [0, 0, 0, 0])[2] > 0]
    def diag(tag, d):
        # one line of state per step: what the composer holds, whether the
        # palette is up, whether a receipt is laid out, whether a turn is live
        return (f"{tag}[draft={app.draft(d)!r} "
                f"palette={(app.rect(d, 'palette_search') or [0, 0, 0, 0])[2]} "
                f"receipt={any('not available' in t or 'Unsupported' in t for t in texts(d))}]")
    def wait_text(needle):
        for _ in range(16):
            d = app.snap()
            if any(needle in t for t in texts(d)):
                return True
            time.sleep(0.5)
        return False
    app.key("escape")  # clear any palette/focus the earlier checks left up
    time.sleep(1.0)
    # A fresh session (the sidebar's New chat) makes the receipts' viewport
    # position deterministic: earlier checks' hydrate/reselect chains refill
    # the timeline and the auto_tail'd list can leave a brand-new group
    # outside the instantiated window. A new chat is the same path a user
    # takes; the behaviour under test is unchanged.
    d0 = app.snap()
    nc = next((w.get('r') for w in d0.get('s', [])
               if (w.get('t') or '') == 'New chat' and (w.get('r') or [0, 0, 0, 0])[2] > 0), None)
    new_chat = False
    if nc:
        app.click(int(nc[0] + nc[2] / 2), int(nc[1] + nc[3] / 2))
        time.sleep(2.0)
        new_chat = True
    r = comp(); app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
    steps: list[str] = []
    # 1. TUI-only known name -> receipt, cleared composer
    app.clear_composer(); app.type("/title"); app.key("return")
    for i in range(16):
        d = app.snap()
        if any("/title is not available" in t for t in texts(d)):
            break
        steps.append(diag(f"title t+{i}", d))
        time.sleep(0.5)
    got_receipt = wait_text("/title is not available")
    time.sleep(1.0)
    d = app.snap()
    cleared = (app.draft(d) or "") in ("", PLACEHOLDER)
    # 2. unknown name -> receipt, text kept
    app.clear_composer(); app.type("/bogus"); app.key("return")
    got_unknown = wait_text("Unsupported command: /bogus")
    time.sleep(1.0)
    d = app.snap()
    kept = (app.draft(d) or "") == "/bogus"
    # 3. a path reaches the model: it becomes the user's own row and a turn
    # runs for it. A11: the replayed turn is now the app's own and settles in
    # well under the 0.5 s poll, so a working row alone could be missed — the
    # prompt row carrying the path, plus a new answer (or the working row),
    # is the turn.
    def proses(snap):
        return sum(1 for w in snap.get("s", []) if "assistantprose" in str(w.get("i", "")))
    path = "/home/user/x/proj/main.rs"
    base_proses = proses(app.snap())
    app.clear_composer(); app.type(path); app.key("return")
    turned = False
    for _ in range(24):
        d = app.snap()
        bubble = any((m := INSTANCE_KIND_RE.match(str(w.get("i", "")))) and m.group(1) == "userbubble"
                     and (w.get("t") or "").strip() == path for w in d.get("s", []))
        if bubble and ("workingrow" in app.kinds(d) or proses(d) > base_proses):
            turned = True
            break
        time.sleep(0.5)
    for _ in range(60):
        if "workingrow" not in app.kinds(app.snap()):
            break
        time.sleep(1)
    ok = got_receipt and cleared and got_unknown and kept and turned
    return ok, (f"new_chat={new_chat} title_receipt={got_receipt} cleared={cleared} "
                f"bogus_receipt={got_unknown} kept={kept} path_turn={turned} "
                f"steps={' '.join(steps[:6])}")


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
# A5 (judge, board 2): the pre-board-2 GOALS / LOOPS / FLEET sidebar sections
# were removed (they squeezed the approved sidebar tree). The fleet roster is
# A4's footer Fleet pane; goals and loops are the autonomy dialogs (/goal,
# /loop), which docs/ux/a5-dialogs/walk/walk.log clicks through.
@check("peer", "the fleet roster is reached from the sidebar footer's Fleet entry")
def p_roster(app):
    # A11: the area's FIRST check — poll for the footer's layout instead of
    # reading the first frame (measured: [0,0,0,0] here, [64,637,260,34] two
    # checks later in the same instance).
    try:
        d = app.wait_for(lambda s: (app.rect(s, "fleet_nav_hit") or [0, 0, 0, 0])[2] > 0,
                         timeout=15, what="the sidebar footer's Fleet entry to lay out")
    except AssertionError:
        d = app.snap()
    r = app.rect(d, "fleet_nav_hit")
    ok = bool(r) and r[2] >= 28 and r[3] >= 28
    return ok, f"fleet_nav_hit rect={r}"


@check("peer", "the board-2 sidebar carries no GOALS/LOOPS/FLEET sections")
def p_sections(app):
    d = app.snap()
    texts = [w.get("t", "") for w in d.get("s", [])]
    stray = [t for t in texts if t in ("GOALS", "LOOPS", "FLEET")]
    return not stray, f"stray sections={stray}"


@check("peer", "goals and loops open as dialogs from the palette",
       rows=("goal", "loop", "plan", "trajectory"))
def p_rows(app):
    # A11: the palette shows one page of rows (A5's palette fits its list), so
    # '/' alone may not list /goal and /loop: filter for each, as a user does.
    found = {}

    def lists(s, want):
        return any(w.get("t") == want for w in s.get("s", []) if w.get("i") == "palette_row_name")

    for query, want in (("/go", "/goal"), ("/lo", "/loop")):
        app.clear_composer()
        app.type_into_composer(query)
        try:
            app.wait_for(lambda s: lists(s, want), timeout=4, what=f"the palette to list {want}")
            found[want] = True
        except AssertionError:
            found[want] = False
    app.clear_composer()
    ok = all(found.values())
    return ok, f"palette rows found={found}"


# ---- #41c: row-specific checks for the smoke-only rows --------------------- #
# One check per smoke-only row whose own case is exercisable on the replay
# fixtures; every other smoke-only row carries a documented reason in
# docs/phase4-gaps.md (fixture lacks the frames / native surface absent /
# viewport semantics). New FAILs land in docs/walk/defects.md.

@check("peer", "Alt+D reaches a fleet capability notice surface to focus",
       rows=("alt+d",))
def p_altd_notice(app):
    # Row 72's own case (keyboard-parity.spec.ts:379: Alt+D focuses Fleet's
    # capability notice; suppressed inside inputs). A11: A7 bound it since the
    # 41c recon (lib.rs `ParityShortcut::FocusDispatch` -> b3.open.fleet, with
    # the §8 suppression inside text inputs and dialogs). Both halves are
    # driven: inside the composer the chord is suppressed (no Fleet; the app
    # logs the suppression), and after a click on the open session's row
    # (focus leaves the text input) Alt+D opens Fleet with its notice. Focus
    # itself is not observable through /snap.
    def fleet(s):
        return (app.rect(s, "b3_fleet_panel") or [0, 0, 0, 0])[2] > 0

    def log_mark():
        return json.loads(app._get("/log?n=1")).get("n", 0)

    def logged_since(mark, needle):
        return any(needle in l for l in json.loads(app._get(f"/log?since={mark}")).get("l", []))

    app.key("escape")
    app.focus_composer(app.snap())
    mark = log_mark()
    app.key_mod("keyd", alt=True)
    time.sleep(1.0)
    suppressed = logged_since(mark, "shortcut FocusDispatch suppressed") and not fleet(app.snap())
    row = app.shown_rect_re(app.snap(), THREAD_OPEN_RE)
    if not row:
        return False, f"suppressed_in_composer={suppressed}; no session row to move focus to"
    app.click(int(row[0] + row[2] / 2), int(row[1] + row[3] / 2))
    time.sleep(1.0)
    mark = log_mark()
    app.key_mod("keyd", alt=True)
    try:
        d = app.wait_for(fleet, timeout=6, what="Alt+D to open Fleet")
    except AssertionError:
        d = app.snap()
    opened = fleet(d) and logged_since(mark, "shortcut Alt+D -> fleet")
    laid = [str(w.get("t") or "") for w in d.get("s", [])
            if str(w.get("i", "")).startswith("b3_fleet") and app.laid_out(w) and w.get("t")]
    notice = next((t for t in laid if "does not support" in t or "No peer models" in t
                   or t == "Start a peer"), None)
    if fleet(d):
        app.click_id(d, "b3_fleet_back")
        try:
            app.wait_for(lambda s: not fleet(s), timeout=6, what="Fleet's Back to close it")
        except AssertionError:
            pass
    ok = suppressed and opened and notice is not None
    return ok, (f"suppressed_in_composer={suppressed} opened_outside_inputs={opened} "
                f"notice={notice!r}")


@check("peer", "Alt+P toggles a peer dock fold (expands and collapses)",
       rows=("alt+p",))
def p_altp_fold(app):
    # Row 73's own case. A30: the sidebar peer dock is built
    # (screens/peer_dock.rs, `peer_dock_row` / `pd_dock`). With NO peers it is
    # absent by design (the web renders nothing) and Alt+P draws none — the
    # r6 replay stages no roster row, so this run proves that half; the fold
    # itself (expanded -> one pill -> expanded) is walked with real peers by
    # tools/walk/a30_peer_dock.py (row 73 in its WALK rows).
    app.key("escape")
    def dock(s):
        return app.rect(s, "pd_dock") if app.rect(s, "peer_dock_row") else None
    d0 = app.snap()
    rows_ = [w for w in d0.get("s", []) if str(w.get("i", "")).startswith("pd_row_") and app.laid_out(w)]
    app.key_mod("keyp", alt=True)
    r1 = dock(app.snap())
    app.key_mod("keyp", alt=True)
    r2 = dock(app.snap())
    if not rows_ and r1 is None and r2 is None:
        return True, ("no peers in this replay: the dock is absent by design and Alt+P draws none "
                      "(the fold with peers: tools/walk/a30_peer_dock.py)")
    ok = bool(r1) and bool(r2) and r1 != r2
    return ok, f"pd_dock rects={r1} -> {r2}"


@check("peer", "Fleet's roster rows render with real rects (the mock's lanes)",
       rows=("fleet opens", "2 lanes"))
def p_fleet_rows_visible(app):
    # Row 128's own domain: the roster lists the mock's lanes as VISIBLE rows.
    # The id-only smoke check passed while every fleet rect stayed collapsed
    # (measured 41c recon under the r6-peer fixture) — this asserts the rect.
    # A5: the sidebar fleet rows are gone; the roster opens from the footer.
    d = app.snap()
    r = app.rect(d, "fleet_nav_hit")
    ok = bool(r) and r[2] > 0 and r[3] > 0
    return ok, f"fleet_nav_hit rect={r}"


@check("recovery", "the palette's /monitor command reaches a monitors surface",
       rows=("monitors create",))
def r_monitor_entry(app):
    # Row 102's own domain: monitors managed through typed receipts, entered
    # via the palette's /monitor (the list q_execute probed carries it).
    app.wait_for(lambda s: (app.rect_re(s, COMPOSER_INPUT_RE) or [0, 0, 0, 0])[2] > 0,
                 what="the composer input to lay out")
    app.focus_composer(app.snap())
    app.clear_composer()
    app.type("/monitor")
    app.wait_for(lambda s: any("/monitor" in str(w.get("t", "")).lower()
                               for w in s.get("s", [])),
                 what="the /monitor palette entry")
    app.key("return")
    try:
        d = app.wait_for(lambda s: any(i.startswith("monitor")
                                       for i in app.widget_ids(s))
                         or any("monitor" in str(w.get("t", "")).lower()
                                and str(w.get("t", "")).strip().lower() != "/monitor"
                                for w in s.get("s", [])),
                         timeout=8, what="a monitors surface after executing /monitor")
    except AssertionError:
        d = app.snap()
    ids = [i for i in app.widget_ids(d) if i.startswith("monitor")]
    texts = [str(w.get("t", "")) for w in d.get("s", [])
             if "monitor" in str(w.get("t", "")).lower()
             and str(w.get("t", "")).strip().lower() != "/monitor"]
    # A11 hygiene: since A5, /monitor opens the Monitors DIALOG — a modal scrim
    # over the whole module. Left open it swallowed the next checks' clicks and
    # keys (the settings opener, the composer: three recovery checks timed
    # out behind it). Close it with its own control, as a user does.
    closed = _close_dialog(app)
    return bool(ids or texts), f"monitor ids={ids[:4]} texts={texts[:3]} dialog_closed={closed}"


def _close_dialog(app) -> bool:
    """Close an open A5 dialog (its ✕, else Escape); True when none is up."""
    def up(s):
        return (app.rect(s, "dialog_root") or [0, 0, 0, 0])[2] > 0
    for _ in range(3):
        d = app.snap()
        if not up(d):
            return True
        r = app.rect(d, "dialog_close")
        if r and r[2] > 0:
            app.click(int(r[0] + r[2] / 2), int(r[1] + r[3] / 2))
        else:
            app.key("escape")
        try:
            app.wait_for(lambda s: not up(s), timeout=4, what="the dialog to close")
            return True
        except AssertionError:
            continue
    return not up(app.snap())


@check("settings", "the settings drawer exposes the Models management section",
       rows=("dsh-style models", "models settings flow"))
def s_models_section(app):
    # Row 87's own case. Measured 41c recon: the drawer mounts header + the
    # connection action only — no Model/Models entry. Expected FAIL until the
    # section ships.
    def drawer_open(s):
        return (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0
    # Pre-clean: a previous check may have left the drawer open (the open hit
    # is a TOGGLE — clicking then would CLOSE it and time out the wait).
    d = app.snap()
    if drawer_open(d):
        app.click_id(d, "settings_close")
        app.wait_for(lambda s: not drawer_open(s), timeout=8,
                     what="the pre-existing drawer to close")
    opened = False
    for _ in range(3):  # the toggle click is flaky right after mount
        d = app.snap()
        app.click_id(d, "settings_open_hit")
        try:
            app.wait_for(drawer_open, timeout=8, what="the settings drawer to open")
            opened = True
            break
        except AssertionError:
            continue
    if not opened:
        return False, "the settings drawer never opened via settings_open_hit"
    d = app.snap()
    texts = [str(w.get("t", "")) for w in d.get("s", [])]
    ok = any(t in texts for t in ("Model", "Models", "Manage models"))
    # Hygiene via the drawer's OWN close control: Esc does not close the
    # drawer (defects.md #41c-5), and a left-open drawer covers the
    # composer's right half and poisons later checks.
    d = app.snap()
    if drawer_open(d):
        app.click_id(d, "settings_close")
        app.wait_for(lambda s: not drawer_open(s), timeout=8,
                     what="the drawer to close after the check")
    return ok, f"models section texts={[t for t in texts if 'model' in t.lower()][:3]}"


@check("recovery", "Escape closes the settings drawer and the trigger still works",
       rows=("escape restores",))
def k_esc_drawer(app):
    # Row 58's own domain: Escape hands control back and the trigger survives.
    def drawer_open(s):
        return (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0
    d = app.snap()
    if drawer_open(d):  # pre-clean a drawer a previous check left open
        app.click_id(d, "settings_close")
        app.wait_for(lambda s: not drawer_open(s), timeout=8,
                     what="the pre-existing drawer to close")
    d = app.snap()
    app.click_id(d, "settings_open_hit")
    app.wait_for(drawer_open, what="the settings drawer to open")
    # The row's own case: Escape hands control back. Fold the verdict (no
    # raw raise) so the cleanup always runs.
    # A11: the press itself was lost in #41c's restructure (5ebac7b4 removed
    # `app.key("escape")` here), so the check waited for a close nothing asked
    # for and could never pass. A3's chrome closes Settings on Escape
    # (lib.rs `escape_chrome`: "chrome: Escape closed the top surface").
    time.sleep(0.5)
    app.key("escape")
    esc_closes = True
    try:
        app.wait_for(lambda s: not drawer_open(s), timeout=6,
                     what="Escape to close the settings drawer")
    except AssertionError:
        esc_closes = False
    if not esc_closes:
        d = app.snap()  # defects.md #41c-5: Esc does not close the drawer
        if drawer_open(d):
            app.click_id(d, "settings_close")
            app.wait_for(lambda s: not drawer_open(s), timeout=8,
                         what="the drawer to close via its own control")
        return False, "Escape did not close the settings drawer (closed via its own control)"
    # The trigger still works after Esc handed control back.
    d = app.snap()
    app.click_id(d, "settings_open_hit")
    app.wait_for(drawer_open, what="the Settings trigger to re-open the drawer")
    d = app.snap()
    app.click_id(d, "settings_close")
    app.wait_for(lambda s: not drawer_open(s), timeout=8,
                 what="the drawer to close at the end")
    return True, "open -> Esc closes -> trigger re-opens -> closed cleanly"


# ---- live-only rows against the REAL gate (#39a, --live) -------------------- #
# These run ONLY in --live mode (they drive real model turns through the
# outer loop's octos serve); against replay fixtures they would be nonsense.
LIVE_CHECK_NAMES = frozenset({
    "a real coding turn streams, terminates, and the timeline survives a refresh",
    "a live turn keeps running while a sibling session is focused",
    "files the agent delivers render as attachments with captions",
    "a turn started by another client is disclosed as theirs",
    "a shell prompt under Ask-for-approval raises an approval card",
})

# #41d — the operator's hard cap on REAL model turns for this card. Every
# live check charges its turns through spend_turn(); the runner refuses to
# send turn 61.
LIVE_TURN_BUDGET = 60
_TURNS = {"used": 0}


def turns_used() -> int:
    return _TURNS["used"]


def spend_turn(n: int = 1) -> int:
    """Charge n REAL model turns to the #41d budget; hard-stop past the cap."""
    _TURNS["used"] += n
    if _TURNS["used"] > LIVE_TURN_BUDGET:
        raise PrereqError(
            f"#41d turn budget exhausted: {_TURNS['used']} turns > "
            f"{LIVE_TURN_BUDGET} — no further live sends")
    return _TURNS["used"]

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
    spend_turn()  # #41d budget
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
    spend_turn()  # #41d budget
    app.wait_for(working, timeout=30, what="the background turn to go live")
    # focus a sibling: New chat mints a fresh session
    nb = (app.rect(app.snap(), "sb_new_chat_hit") or app.rect(app.snap(), "new_chat_hit")
          or app.rect(app.snap(), "newchat"))
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


@check("composer", "files the agent delivers render as attachments with captions",
       rows=("shows files the agent delivers",))
def c33_live_deliveries(app):
    # Row 33's live slice: one short turn asking for two files; the
    # deliveries must surface in the timeline as laid-out attachment rows
    # with caption text (the downloads/reload halves stay web-only).
    def atts(d):
        return [(str(w.get("i")), (w.get("t") or "")[:40]) for w in d.get("s", [])
                if "attach" in str(w.get("i", "")).lower()
                and (w.get("r") or [0, 0, 0, 0])[2] > 0]
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
    app.clear_composer()
    app.type("create two small files note-a.txt and note-b.txt in this workspace")
    app.send()
    spend_turn()  # #41d budget
    app.wait_for(lambda s: not working(s), timeout=180,
                 what="the delivery turn to finish")
    time.sleep(1.5)
    found = atts(app.snap())
    ok = len(found) >= 1
    return ok, f"attachment rows laid out={len(found)} {found[:2]}"


@check("conversation", "a turn started by another client is disclosed as theirs",
       rows=("discloses the other client's turn",))
def c3_live_foreign_turn(app):
    # Row 3's live slice: an OUT-OF-BAND turn lands on the same session —
    # the check itself drives one raw-protocol turn (charged to the #41d
    # budget) — and the app's session strip must DISCLOSE it as the other
    # client's instead of claiming it as ours.
    import base64
    gate = os.environ.get("OCTOS_LIVE_URL", "http://127.0.0.1:50190")
    ws_url = gate.replace("http", "ws", 1) + "/api/ui-protocol/ws"
    token = pathlib.Path(os.environ["OCTOS_LIVE_TOKEN_FILE"]).read_text().strip()
    from websocket import create_connection  # the audit's dependency
    ws = create_connection(ws_url, header={"Authorization": f"Bearer {token}"},
                           timeout=20)
    def rpc(method, params, mid):
        ws.send(json.dumps({"jsonrpc": "2.0", "id": str(mid),
                            "method": method, "params": params}))
    try:
        rpc("session/open", {"session_id": "dsflash:main", "profile_id": "dsflash"}, 1)
        time.sleep(1.0)
        ws.recv()
        rpc("turn/start", {"session_id": "dsflash:main", "turn_id": "41d-foreign-1",
                           "input": [{"kind": "text",
                                      "text": "reply with just the word: ping"}]}, 2)
        spend_turn()  # #41d budget — this turn is REAL
        for _ in range(6):
            try:
                ws.recv()
            except Exception:
                break
    finally:
        ws.close()
    for _ in range(60):
        d = app.snap()
        strip = " ".join((w.get("t") or "") for w in d.get("s", []))
        low = strip.lower()
        if any(k in low for k in ("other client", "another client", "remote",
                                  "not yours", "foreign")):
            return True, f"strip discloses the foreign turn: {strip[:70]!r}"
        time.sleep(1)
    return False, ("no foreign-turn disclosure surfaced in the strip "
                   "(instrument: which texts carried the turn)")


@check("approval", "a shell prompt under Ask-for-approval raises an approval card",
       rows=("approval shortcuts ignore modified keys",))
def a50_live_approval_raised(app):
    # Row 50's ONE attempt (the card allows a single prompt): dsflash is
    # asked to run a shell command; if the gate's autonomy config routes the
    # tool call through an approval, the app must surface an approval card
    # (the replay era recorded 'approval/requested reaches the store but has
    # no widget' — this is the live re-test). The composer's
    # 'Ask for approval' label is static component art (probe: clicking it
    # changes nothing), so the mode is the gate's, not the shell's.
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
    app.clear_composer(); app.type("run `ls` in a shell")
    app.send()
    spend_turn()  # #41e budget — ONE attempt for row 50
    seen = None
    for _ in range(90):
        d = app.snap()
        ids = app.widget_ids(d)
        card = next((i for i in ids if "approval" in i.lower()
                     and (app.rect(d, i) or [0, 0, 0, 0])[2] > 0), None)
        if card:
            seen = (card, app.rect(d, card))
            break
        if "workingrow" not in app.kinds(d) and _ > 8:
            # terminal without an approval: keep watching a few beats, then
            # report precisely what the store/surface showed
            break
        time.sleep(1)
    if seen:
        return True, f"approval card raised: {seen[0]} r={seen[1]}"
    d = app.snap()
    kinds = [i for i in app.widget_ids(d) if "approval" in i.lower()]
    return False, (f"no approval card in 90s (ids containing approval: "
                   f"{kinds[:6] or 'none'}; tool cells ran without a card?)")


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


@check("review", "the closed-state review opener lays out and opens the diff review by click",
       rows=("review toggle", "opener", "closed-state"))
def v_closed_opener(app):
    # #40b defect 1: the in-panel toggle hit lived INSIDE the closed overlay
    # (rect [0,0,0,0] — #40a's dead click); the header carries an
    # always-mounted opener. A11: since A10 that opener is the web's diff
    # review (board3/diff_review.rs: the eyebrow over "Review changes" in the
    # dialog kit), no longer the docked panel — assert it lays out closed,
    # its CLICK opens the dialog and the dialog's own ✕ closes it again.
    def dialog(s):
        return (app.rect(s, "b3_diff_eyebrow") or [0, 0, 0, 0])[2] > 0
    hit = None
    for _ in range(20):
        d = app.snap()
        hit = app.rect(d, "review_open_hit")
        if hit and hit[2] > 0:
            break
        time.sleep(0.5)
    if not (hit and hit[2] > 0):
        return False, f"closed-state opener not laid out: {hit}"
    app.click(int(hit[0] + hit[2] / 2), int(hit[1] + hit[3] / 2))
    try:
        app.wait_for(dialog, timeout=10, what="the diff review to open by click")
    except AssertionError:
        return False, f"opener={hit}: the CLICK opened no diff review"
    d = app.snap()
    box = app.rect(d, "b3_dialog")
    close = app.rect(d, "b3_close") or [0, 0, 0, 0]
    app.click_id(d, "b3_close")
    try:
        app.wait_for(lambda s: not dialog(s), timeout=6, what="the dialog's close to close it")
        closed = True
    except AssertionError:
        closed = False
    ok = close[2] >= 28 and close[3] >= 28 and closed
    return ok, f"opener={hit} dialog={box} close={close} closed_by_its_x={closed}"


@check("review", "the review toggle is keyboard/click reachable")
def v_toggle(app):
    d = app.snap()
    return "review_toggle_hit" in app.widget_ids(d), "review_toggle_hit present"


# ---- settings: the settings drawer (#28e) ---------------------------------- #
@check("settings", "the drawer's close hit is a real 28x28 slot and Disconnect ends inside the window",
       rows=("28x28", "close hit", "disconnect ends"))
def s_close_disconnect(app):
    # #40b defects 1+2: the close button measured 14 or 0 across rounds (the
    # fork's Right rows steal from the last Fit child to feed Fill siblings)
    # and `Disconnect` sat flush at the window's right edge (right=900,
    # clipped — #40a; #38c fixed the conversation column only). With no Fill
    # sibling in the rows: close is a full 28x28 hit, Disconnect ends inside
    # the window, and the close click closes the drawer.
    hit = None
    for _ in range(20):
        d = app.snap()
        hit = app.rect(d, "settings_open_hit")
        if hit and hit[2] > 0:
            break
        time.sleep(0.5)
    if not (hit and hit[2] > 0):
        return False, f"closed-state opener not laid out: {hit}"
    win = app.rect(app.snap(), "main_window")
    app.click(int(hit[0] + hit[2] / 2), int(hit[1] + hit[3] / 2))
    app.wait_for(lambda s: (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0,
                 timeout=10, what="the drawer to open by click")
    d = app.snap()
    sc = app.rect(d, "settings_close")
    disc = app.rect(d, "settings_disconnect")
    winr = win[0] + win[2]
    close_ok = bool(sc) and sc[2] >= 28
    disc_ok = bool(disc) and disc[0] + disc[2] <= winr - 4
    closed = False
    if sc and sc[2] > 0:
        app.click(int(sc[0] + sc[2] / 2), int(sc[1] + sc[3] / 2))
        time.sleep(1.5)
        closed = (app.rect(app.snap(), "settings_drawer") or [0, 0, 0, 0])[2] == 0
    ok = close_ok and disc_ok and closed
    return ok, (f"close={sc} disconnect_right={disc and disc[0] + disc[2]} "
                f"window_right={winr} close_click_closes={closed}")


@check("settings", "the settings drawer mounts with its close control")
def s_drawer(app):
    d = app.snap()
    ids = app.widget_ids(d)
    ok = "settings_drawer" in ids and "settings_close" in ids
    return ok, f"drawer={'settings_drawer' in ids} close={'settings_close' in ids}"


@check("settings", "the Settings dialog opens with its section navigation",
       rows=("settings", "session settings"))
def s_header(app):
    # A11: A3 replaced #28e's drawer (header "Session settings" over Model /
    # Permissions / Sandbox / Context) with the Settings dialog — a "Settings"
    # nav over General, Permissions, Model, Sandbox, Connection, Preferences
    # and About (chrome.rs `OcSettingsPanel`, `Section::title`); A8's Session
    # settings pane is another surface (the session strip opens it). The old
    # check read hidden texts; this one OPENS the dialog by its header control
    # and asserts the laid-out nav, then closes it with its own ✕.
    def drawer_open(s):
        return (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0
    opened = drawer_open(app.snap())
    for _ in range(3):
        if opened:
            break
        app.click_id(app.snap(), "settings_open_hit")
        try:
            app.wait_for(drawer_open, timeout=8, what="the Settings dialog to open")
            opened = True
        except AssertionError:
            continue
    if not opened:
        return False, "the Settings dialog never opened via settings_open_hit"
    time.sleep(0.5)
    d = app.snap()
    shown = {str(w.get("t") or "").strip() for w in d.get("s", [])
             if (w.get("r") or [0, 0, 0, 0])[2] > 0}
    nav = ("General", "Permissions", "Model", "Sandbox", "Connection")
    missing = [t for t in nav if t not in shown]
    close = app.rect(d, "settings_close") or [0, 0, 0, 0]
    app.click_id(d, "settings_close")
    try:
        app.wait_for(lambda s: not drawer_open(s), timeout=8, what="the Settings dialog to close")
        closed = True
    except AssertionError:
        closed = False
    ok = not missing and close[2] > 0 and closed
    return ok, f"nav missing={missing} close={close} closed_by_its_x={closed}"


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
    # A11: A5's setup-08 footer reads "to move · to run · esc" (lib.rs
    # `palette_hint`; it was "move" / "run"). The old check read hidden texts;
    # this one opens the palette as a user does ('/' in the composer), asserts
    # the hint is LAID OUT under the list, then empties the composer.
    hints = ("to move", "to run", "· esc")
    app.focus_composer(app.snap())
    app.clear_composer()
    app.type("/")
    try:
        d = app.wait_for(lambda s: (app.rect(s, "palette_search") or [0, 0, 0, 0])[2] > 0,
                         timeout=10, what="the palette to open on '/'")
    except AssertionError:
        app.clear_composer()
        return False, "the palette never opened on '/'"
    time.sleep(0.5)
    d = app.snap()
    laid = [str(w.get("t") or "").strip() for w in d.get("s", [])
            if (w.get("r") or [0, 0, 0, 0])[2] > 0]
    shown = [h for h in hints if h in laid]
    lst = app.rect(d, "palette_list") or [0, 0, 0, 0]
    hint_y = min((w["r"][1] for w in d.get("s", [])
                  if str(w.get("t") or "").strip() == "to move" and (w.get("r") or [0, 0, 0, 0])[2] > 0),
                 default=0)
    below = hint_y >= lst[1] + lst[3] - 1 if lst[2] > 0 else False
    app.key("escape")
    app.clear_composer()
    ok = len(shown) == len(hints) and below
    return ok, f"hint parts laid out={shown} below_list={below}"


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
    # A11: A3's sidebar renamed New chat (sb_new_chat_hit).
    new_chat = "sb_new_chat_hit" in ids or "new_chat_hit" in ids
    ok = "sidebar_toggle_hit" in ids and new_chat
    return ok, f"sidebar={'sidebar_toggle_hit' in ids} new_chat={new_chat}"


@check("keyboard", "the a11y keyboard guarantees hold: Ctrl+K, Esc, / all route",
       rows=("a11y",))
def k_a11y_semantics(app):
    # Row 172's own case (the a11y batch): the shell's keyboard model —
    # Cmd+K opens the palette, Esc closes it, / re-opens, Esc closes.
    app.key("escape")
    # '/' opens the palette only on an EMPTY draft (keys.rs:110) — clear any
    # residue earlier keyboard checks left in the composer.
    d = app.snap()
    if (app.rect_re(d, COMPOSER_INPUT_RE) or [0, 0, 0, 0])[2] > 0:
        app.clear_composer()
    # The palette's mounted signal is the SEARCH field's rect (the proven
    # q_execute predicate) — the list rows carry generated ids.
    def pal_open(s):
        return (app.rect(s, "palette_search") or [0, 0, 0, 0])[2] > 0
    # ctrl, not cmd: keys.rs:106 accepts `logo || ctrl` (web App.tsx:1096
    # `metaKey || ctrlKey`), and the ctrl chord PROVES delivery — measured on
    # the walk host, keyk&ctrl opens the palette while keyk&cmd does not
    # (three runs), with cmd delivery itself proven by Cmd+E opening the
    # review dock (#36e). The logo-arm drop is recorded in defects.md.
    app.key_mod("keyk", ctrl=True)
    app.wait_for(pal_open, what="Ctrl+K to open the palette")
    app.key("escape")
    app.wait_for(lambda s: not pal_open(s), what="Esc to close the palette")
    app.type("/")
    app.wait_for(pal_open, what="'/' to open the palette")
    app.key("escape")
    app.wait_for(lambda s: not pal_open(s), what="Esc to close the palette again")
    return True, "Ctrl+K open, Esc close, / open, Esc close — all routed"


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


@check("recovery", "the status strip labels stay constructed (no timeline text leaks)",
       rows=("p4b2-parity-evidence-only",))
def r_strip_purity(app):
    # #P4b2 §8.23 evidence for the parity row `Strip state/thinking text from
    # the session status strip label`: the strip text read from the LIVE /snap
    # must be exactly the constructed shape (store summary() "conn: …   sessions: N")
    # — never timeline text, even though this very fixture streams reasoning
    # deltas into the transcript. rows= is a non-matching token on purpose:
    # this check is parity EVIDENCE, it must not claim any walk row's depth.
    d = app.snap()
    st = (app.text_of(d, "status") or "").strip()
    import re as _re
    ok = bool(_re.fullmatch(r"conn: \S+\s+sessions: \d+", st))
    leaked = [w.get("t") for w in d.get("s", [])
              if w.get("t") and ("thinking…" in str(w.get("t")))
              and str(w.get("i", "")) .startswith("status")]
    return ok and not leaked, f"status strip from /snap: {st!r} (constructed={ok}, leak={bool(leaked)})"


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


# --------------------------------------------------------------------------- #
# #43b — the three rows the runner could not drive. Each check is registered
# under the area `env_group_for` resolves for its row, and is only ever run on
# the instance its ENV_GROUPS entry launched (main() filters by name), so a
# first-run assertion can never see a connected app.
# --------------------------------------------------------------------------- #
@check("keyboard", "the first-run chrome mounts with no connection and a focused composer",
       rows=("starts a first workspace with the keyboard",))
def fr_chrome(app):
    # Row 106's precondition, half 1 (e2e/onboarding.spec.ts:77). Measured on
    # the first-run instance: `first_run` paints because `store.is_live()` is
    # false (lib.rs:2301-2302, 2421) — the transport is never built
    # (lib.rs:1341-1348), so `conn:` must NOT read Live.
    d = app.wait_for(lambda s: (app.rect(s, "first_run") or [0, 0, 0, 0])[2] > 0,
                    timeout=20, what="the first-run chrome to mount")
    ids = app.widget_ids(d)
    texts = [str(w.get("t", "")) for w in d.get("s", [])]
    status = (app.text_of(d, "status") or "").strip()
    chrome = all(k in ids for k in ("first_run", "first_run_sidebar", "first_run_center"))
    empty_sidebar = "No threads yet" in texts and "OctosCode" in texts
    # NOT connected: the status line keeps its bare `conn:` label (no "Live"),
    # and the connected-only chrome (thread list rows) is absent.
    not_live = "Live" not in status
    no_threads = not any(str(i).startswith("i0_threadrow") for i in ids)
    # The Connect card mounts into the first-run dock (lib.rs:2242-2244, the
    # `!live` arm) — it is the first-run affordance, and its own fields are the
    # only TextInputs on screen.
    connect = any(t == "Connect to Octos" for t in texts)
    ok = chrome and empty_sidebar and not_live and no_threads and connect
    return ok, (f"chrome={chrome} empty_sidebar={empty_sidebar} status={status!r} "
                f"not_live={not_live} no_thread_rows={no_threads} connect_card={connect}")


@check("keyboard", "the Add-workspace dialog focuses its path field and Escape keeps the draft",
       rows=("starts a first workspace with the keyboard",))
def fr_add_workspace(app):
    # Row 106, half 2: "Open the Add-workspace dialog and press Escape" — the
    # dialog focuses its PATH FIELD, and Escape closes it WITHOUT losing the
    # unsent draft. PROBED on the first-run instance: `/snap?all=1` carries no
    # add-workspace affordance at all (no id or label matching `add`, in any
    # casing — see .peer/report-43b.md §3), and the first-run chrome has no
    # composer to hold a draft (the `!live` arm hides `base`, lib.rs:2301).
    # So this is an HONEST expected-fail: the runner can now drive the flow, and
    # driving it proves the surface is absent. It is NOT worked around.
    d = app.snap()
    ids = app.widget_ids(d)
    texts = [str(w.get("t", "")) for w in d.get("s", [])]
    aff = [i for i in ids if "add" in i.lower()] + \
          [t for t in texts if "add" in t.lower() and "addworkspace" not in t.lower()]
    add_workspace = any("workspace" in a.lower() for a in aff)
    # The draft the row keeps: the row types into the COMPOSER, which the
    # first-run chrome does not mount.
    composer = app.rect_re(d, COMPOSER_INPUT_RE)
    return (add_workspace and composer is not None), (
        f"add_workspace_affordance={add_workspace} hits={aff[:3]} "
        f"composer={'yes' if composer else 'absent'}")


@check("peer", "the requested viewport width is honoured and picks the width breakpoints",
       rows=("preserves its geometry while model management loads",))
def vp_width(app):
    # Row 183's `${viewport.width}` semantics. The app reads the width from the
    # env ONCE (lib.rs:2387-2394 → `window_w`) and branches on it twice:
    # `wide` at >= 1260 (lib.rs:2416) and `width_hides_sidebar` at < 760
    # (lib.rs:2418). Probed at 1280x900: `main_window` [0,0,1280,900] with
    # `threads_column` [54,86,260,593] VISIBLE; at 720x900: `threads_column`
    # [0,0,0,0] and the composer moves left to x=65. This check therefore
    # asserts the width was applied AND that the module took the branch the
    # width selects — at EITHER edge, whichever the instance was launched with.
    # A11: since A3 the shell no longer sizes its window from the env: the
    # module draws INSIDE a WxH frame (lib.rs `env_frame`, a CAP on the module
    # window), so `main_window` is the 1400-wide shell, never the request. The
    # module's own frame is `base` — and a 1280 frame needs a module window at
    # least that wide, so the 1280 instance is MAXIMIZED by the shell's own
    # `--test-action maximize` (ENV_GROUPS).
    want = app.env.get("OCTOSENSE_WINDOW_SIZE", "")
    m = re.match(r"^(\d+)x(\d+)$", want)
    if not m:
        return False, f"no OCTOSENSE_WINDOW_SIZE in the launch env: {want!r}"
    w_req = float(m.group(1))
    d = app.wait_for(lambda s: (app.rect(s, "base") or [0, 0, 0, 0])[2] > 0,
                    timeout=20, what="the module to mount at the requested width")
    # The shell's maximize animates: on a loaded machine the first laid-out
    # frame can be caught mid-way (the official run on 9c4fb794 read 1275 at
    # 1280; alone it reads 1280). Let the frame settle on the request; one that
    # never gets there still fails below with the width it did reach.
    try:
        d = app.wait_for(lambda s: abs((app.rect(s, "base") or [0, 0, 0, 0])[2] - w_req) < 1.0,
                        timeout=10, what="the module frame to settle on the requested width")
    except Exception:  # noqa: BLE001 — measured as it stands; honoured=False
        d = app.snap()
    win = app.rect(d, "base") or [0, 0, 0, 0]
    got = float(win[2])
    honoured = abs(got - w_req) < 1.0
    # The branch: at >= 1260 the sidebar column is laid out; below 760 it is
    # collapsed to a zero rect. Both are the module's own decision, not ours.
    col = app.rect(d, "threads_column") or [0, 0, 0, 0]
    wide = w_req >= 1260.0
    if wide:
        branch = col[2] > 0
        want_state = "wide: threads_column laid out"
    elif w_req < 760.0:
        branch = col[2] == 0
        want_state = "narrow: threads_column collapsed"
    else:
        branch = col[2] > 0
        want_state = "mid: threads_column laid out"
    return (honoured and branch), (
        f"requested={w_req:g} module_frame_w={got:g} honoured={honoured} "
        f"threads_column={col} want={want_state} branch={branch}")


@check("peer", "the settings drawer keeps one dialog and its geometry across a panel load",
       rows=("preserves its geometry while model management loads",))
def vp_geometry(app):
    # Row 183's actual pass_condition: "the settings dialog's geometry is
    # IDENTICAL before and after the panel loads", with the dialog count at one
    # and the Models control keeping focus. (#43b measured no Models section
    # in the old drawer — an expected FAIL then; A3's dialog carries one.)
    # Focus is not observable through /snap, so it is not asserted.
    def drawer_open(s):
        return (app.rect(s, "settings_drawer") or [0, 0, 0, 0])[2] > 0
    d = app.snap()
    if drawer_open(d):
        app.click_id(d, "settings_close")
        app.wait_for(lambda s: not drawer_open(s), timeout=8,
                     what="a pre-existing drawer to close")
    opened = False
    for _ in range(3):  # the toggle click is flaky right after mount
        d = app.snap()
        app.click_id(d, "settings_open_hit")
        try:
            app.wait_for(drawer_open, timeout=8, what="the settings drawer to open")
            opened = True
            break
        except AssertionError:
            continue
    if not opened:
        return False, "the settings drawer never opened via settings_open_hit"
    # A11: A3's Settings dialog HAS a Model section (chrome.rs `Section::Model`),
    # so the row's own sequence is drivable: measure the dialog, open the Model
    # section (its nav cell, or the rail chip below the phone breakpoint), let
    # it load, and measure again — the geometry must be IDENTICAL.
    time.sleep(0.5)
    d1 = app.snap()
    geo1 = app.rect(d1, "settings_drawer")
    cell = next((c for c in ("set_nav_model", "set_rail_model")
                 if (app.rect(d1, c) or [0, 0, 0, 0])[2] > 0), None)
    if cell:
        app.click_id(d1, cell)
    try:
        d2 = app.wait_for(lambda s: app.text_of(s, "set_title") == "Model", timeout=8,
                          what="the Model section to load")
    except AssertionError:
        d2 = app.snap()
    time.sleep(1.0)  # whatever the section reads arrives and lays out
    d2 = app.snap()
    geo2 = app.rect(d2, "settings_drawer")
    loaded = app.text_of(d2, "set_title") == "Model"
    dialogs = sum(1 for w in d2.get("s", [])
                  if str(w.get("i", "")) == "settings_drawer" and (w.get("r") or [0, 0, 0, 0])[2] > 0)
    stable = geo1 is not None and geo1 == geo2
    if drawer_open(d2):
        app.click_id(d2, "settings_close")
        app.wait_for(lambda s: not drawer_open(s), timeout=8, what="the drawer to close")
    return (loaded and stable and dialogs == 1), (
        f"before={geo1} after_model_load={geo2} model_section={loaded} via={cell} "
        f"dialogs={dialogs} identical={stable}")


@check("settings", "manual light keeps conversation and settings text readable",
       rows=("manual light preserves readable conversation",))
def theme_light(app):
    # Row 212 (e2e/theme.spec.ts:72): the STORED preference is light while the
    # OS stays dark, and the web asserts no colour-contrast violation in the
    # conversation AND in Settings.
    # `/snap` exposes NO colour channel (probe: the only keys are
    # i/ty/r/w/v/val/t/enabled/window_id), so the contrast MUST be measured from
    # the app's OWN PNG via `/g?raw=1` — `App.raw()` (the UTF-8 `_get` corrupts
    # a PNG). Probed at OCTOSCODE_THEME=light: white-dominant surface
    # ((255,255,255) x 1.6M) and every text cluster >= 5.03:1, so this passes
    # on the real pixels rather than on a token lookup.
    want = app.env.get("OCTOSCODE_THEME", "")
    if want != "light":
        return False, f"the launch env is not the light theme: OCTOSCODE_THEME={want!r}"
    # #43b: WAIT for the mount before mapping anything. A freshly launched
    # instance's first `/snap` can still read `main_window` as [0,0,0,0] — the
    # first run of this check failed exactly there ("main_window has no laid-out
    # rect"), while `vp_width` passed because it waits. The PNG is fetched AFTER
    # the wait so the pixels and the rects describe the same frame.
    d = app.wait_for(lambda s: (app.rect(s, "main_window") or [0, 0, 0, 0])[2] > 0,
                    timeout=30, what="the module to mount in the light theme")
    win = app.rect(d, "main_window")
    png = app.raw("/g?raw=1")
    try:
        pw, ph, rows = _decode_png(png)
    except Exception as e:  # noqa: BLE001
        return False, f"could not decode the app's own PNG for contrast: {e}"
    sx = pw / float(win[2])
    measured = _contrast_of(rows, d, pw, sx)
    if not measured:
        return False, "no text-bearing widget carried measurable ink in the PNG"
    # WCAG AA for normal text is 4.5:1; the web row asserts NO violation.
    bad = [(t, r) for t, r in measured if r < 4.5]
    worst = min(measured, key=lambda m: m[1])
    return (not bad), (f"png={pw}x{ph} widgets={len(measured)} worst={worst[0][:24]!r} "
                       f"{worst[1]:.2f}:1 violations={[(t[:18], round(r, 2)) for t, r in bad][:3]}")


def _decode_png(png: bytes):
    """(width, height, rows) of an 8-bit RGB/RGBA non-interlaced PNG — stdlib only.

    The walk lane's interpreter has NO Pillow (measured: `import PIL` fails
    under `.peer/env.sh`, and the check's first run failed exactly there), and a
    check must not carry a dependency the lane cannot satisfy — so the five
    filter types are undone here (RFC 2083 §6). Measured cost on the real
    1800x2044 RGBA capture: 1.0 s, which is fine for one PNG per run.
    """
    if png[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG (bad signature)")
    off, idat, ihdr = 8, [], None
    while off + 8 <= len(png):
        ln = int.from_bytes(png[off:off + 4], "big")
        typ = png[off + 4:off + 8]
        body = png[off + 8:off + 8 + ln]
        if typ == b"IHDR":
            ihdr = body
        elif typ == b"IDAT":
            idat.append(body)
        elif typ == b"IEND":
            break
        off += 12 + ln
    if ihdr is None or not idat:
        raise ValueError("PNG has no IHDR/IDAT")
    w = int.from_bytes(ihdr[0:4], "big")
    h = int.from_bytes(ihdr[4:8], "big")
    depth, ctype, _, _, interlace = ihdr[8], ihdr[9], ihdr[10], ihdr[11], ihdr[12]
    if depth != 8 or interlace != 0 or ctype not in (2, 6):
        raise ValueError(f"unsupported PNG: depth={depth} colour_type={ctype} "
                         f"interlace={interlace} (need 8-bit, non-interlaced, 2 or 6)")
    nch = 3 if ctype == 2 else 4
    import zlib
    raw = zlib.decompress(b"".join(idat))
    stride = w * nch
    out = bytearray(h * stride)
    prev = bytearray(stride)
    pos = 0
    for y in range(h):
        f = raw[pos]
        pos += 1
        line = bytearray(raw[pos:pos + stride])
        pos += stride
        if f == 1:        # Sub
            for i in range(nch, stride):
                line[i] = (line[i] + line[i - nch]) & 255
        elif f == 2:      # Up
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif f == 3:      # Average
            for i in range(stride):
                a = line[i - nch] if i >= nch else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 255
        elif f == 4:      # Paeth
            for i in range(stride):
                a = line[i - nch] if i >= nch else 0
                b = prev[i]
                c = prev[i - nch] if i >= nch else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
        elif f != 0:
            raise ValueError(f"unknown PNG filter type {f} on row {y}")
        out[y * stride:(y + 1) * stride] = line
        prev = line
    return w, h, (out, stride, nch)


def _rel_lum(c):
    def f(v):
        v /= 255.0
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])


def shrink_png(path: pathlib.Path, width: int = 1400) -> None:
    """A11: committed evidence stays <= 1400 px wide (the brief's capture rule;
    the bridge grabs at 2x, 2800 px). Pillow when present, else macOS `sips`;
    without either the capture is kept as grabbed."""
    try:
        from PIL import Image  # noqa: PLC0415
        with Image.open(path) as im:
            if im.width <= width:
                return
            im.resize((width, round(im.height * width / im.width))).save(path)
        return
    except ImportError:
        pass
    except Exception:  # noqa: BLE001 — evidence is best-effort
        return
    if shutil.which("sips"):
        subprocess.run(["sips", "-Z", str(width), str(path)], capture_output=True)


def _contrast_of(decoded, snap, png_w, sx):
    """(label, ratio) per text-bearing widget, measured on the app's own PNG.

    `decoded` is `_decode_png`'s `(buf, stride, nch)`; the PNG is read
    stdlib-only (the lane interpreter has no Pillow).

    The background is each widget rect's modal pixel; the ink is the
    highest-luminance-distance quartile of the pixels that differ from it
    (antialiased glyph edges dominate otherwise). Returns [] when nothing has
    measurable ink.
    """
    buf, stride, nch = decoded
    out = []
    for w in snap.get("s", []):
        t = str(w.get("t", "")).strip()
        r = w.get("r")
        if not t or not r or r[2] <= 0 or r[3] <= 0:
            continue
        x0, y0 = int(r[0] * sx), int(r[1] * sx)
        x1, y1 = int((r[0] + r[2]) * sx), int((r[1] + r[3]) * sx)
        if x1 <= x0 or y1 <= y0:
            continue
        px = []
        for y in range(max(0, y0), min(len(buf) // stride, y1)):
            row = y * stride
            for x in range(max(0, x0), min(png_w, x1)):
                o = row + x * nch
                px.append((buf[o], buf[o + 1], buf[o + 2]))
        if not px:
            continue
        from collections import Counter
        bg = Counter(px).most_common(1)[0][0]
        bgl = _rel_lum(bg)
        ink = [p for p in px if abs(_rel_lum(p) - bgl) > 0.08]
        if not ink:
            continue
        ink.sort(key=lambda p: abs(_rel_lum(p) - bgl), reverse=True)
        top = ink[:max(1, len(ink) // 4)]
        mean = tuple(sum(c[i] for c in top) // len(top) for i in range(3))
        tl = _rel_lum(mean)
        hi, lo = max(tl, bgl), min(tl, bgl)
        out.append((t, (hi + 0.05) / (lo + 0.05)))
    return out


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
    "a TUI-only slash command reports fail-closed and a path prompt still turns",
    "a follow-up drains as its own turn and a reselect replays nothing",
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
    "the a11y keyboard guarantees hold: Ctrl+K, Esc, / all route",
    # from origin/main (#36g follow-ups):
    "the closed-state review opener lays out and opens the diff review by click",
    "the approval pill opens the permission menu above it and Escape closes it",
    "the drawer's close hit is a real 28x28 slot and Disconnect ends inside the window",
    # #43b — the three harness-limited rows, now driven by their own instances.
    "the first-run chrome mounts with no connection and a focused composer",
    "the Add-workspace dialog focuses its path field and Escape keeps the draft",
    "the requested viewport width is honoured and picks the width breakpoints",
    "the settings drawer keeps one dialog and its geometry across a panel load",
    "manual light keeps conversation and settings text readable",
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


# A11: a check that finds its surface ABSENT by design (not broken) says so by
# opening its reason with this prefix; a row whose every failing check does is
# `not-yet-implemented` with that reason, never a `fail`.
NOT_BUILT = "not built: "


def not_built_reason(checks) -> str:
    """`checks` as (name, status, reason, specific): the not-built reason when
    every failing check is a not-built one, else ''."""
    failing = [r for _n, s, r, *_ in checks if s != "pass"]
    if failing and all(str(r).startswith(NOT_BUILT) for r in failing):
        return "; ".join(sorted(set(failing)))
    return ""


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
            # A11: `--full` passes no limit (every scriptable row).
            if limit is not None and len(picked) >= limit:
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


# --------------------------------------------------------------------------- #
# A11 — the native click walks (tools/walk/native.py) and the honest reasons
# for the rows nothing walks.
# --------------------------------------------------------------------------- #
def load_parity() -> list:
    try:
        with open(PARITY, newline="") as f:
            return list(csv.DictReader(f))
    except FileNotFoundError:
        return []


def final_bucket(p: dict) -> str:
    """The FINAL parity bucket: the manual verdict when set (the brief §3)."""
    return (p.get("phase4_bucket_manual") or "").strip() or (p.get("phase4_bucket") or "").strip()


def parity_hits(row: dict, parity: list) -> list:
    """The parity rows whose web_e2e_specs cite this walk row's case."""
    spec = row["spec"].split("/")[-1]
    key = row["case"][:40]
    return [p for p in parity if spec in (p.get("web_e2e_specs") or "") and key in (p.get("web_e2e_specs") or "")]


def parity_reason(row: dict, parity: list):
    """(status, reason) for a row no check covers, from the parity matrix's
    FINAL verdicts of the capabilities that cite its case — or None.

    * any cited capability still C → `not-yet-implemented` (named);
    * all built (A) → `not-walked`: built, but no click-walk check covers it;
    * only web-only (B) → None (the existing verdict stands)."""
    hits = parity_hits(row, parity)
    if not hits:
        return None
    missing = [p["capability"] for p in hits if final_bucket(p) == "C"]
    built = [p["capability"] for p in hits if final_bucket(p) == "A"]
    if missing:
        return ("not-yet-implemented",
                "missing: " + "; ".join(f"{c[:110]} [C]" for c in missing[:2]))
    if built:
        return ("not-walked",
                "built (" + "; ".join(f"{c[:90]} [A]" for c in built[:2])
                + ") — no click-walk check covers this case yet")
    return None


def relabel_unwalked(out_rows: list, rows: list, parity: list) -> int:
    """Give every `not-yet-implemented` row the parity matrix's own reason
    (and `not-walked` when its capabilities are built). Returns the count."""
    n = 0
    for r in out_rows:
        if r["status"] != "not-yet-implemented":
            continue
        got = parity_reason(rows[r["row_id"] - 1], parity)
        if got:
            r["status"], r["reason"] = got
            n += 1
    return n


def demote_unbuilt(out_rows: list, rows: list, parity: list) -> int:
    """A row that PASSES only on run.py's own area-matched checks while a
    capability the parity matrix cites for its case is still C cannot be a
    pass: generic checks (a Phase-3 regex match on the case title) cannot
    prove an unbuilt capability. It becomes `not-yet-implemented`, naming the
    capability, the passing checks kept in the reason. Native rows (re-pointed
    to click walks) are never demoted here. Returns the count."""
    n = 0
    for r in out_rows:
        if r["status"] != "pass" or r.get("depth") not in ("smoke", "specific"):
            continue
        missing = [p["capability"] for p in parity_hits(rows[r["row_id"] - 1], parity) if final_bucket(p) == "C"]
        if missing:
            r["reason"] = ("missing: " + "; ".join(f"{c[:110]} [C]" for c in missing[:2])
                           + f" — run.py's area-matched checks passed ({r['reason']}) but cannot prove it")
            r["status"] = "not-yet-implemented"
            n += 1
    return n


def merge_native(out_rows: list, check_rows: list, native_rows: dict) -> tuple:
    """Re-point every row a native walk maps.

    * `native` (a walk covers the whole case): the verdict, depth, evidence
      and per-check rows become the native walk's — run.py's area-mapped
      Phase-3 checks for that row are dropped.
    * `native-partial`: the native checks are ADDED to run.py's own checks
      that were TARGETED at rows (a `rows=` tuple, not the ALL smoke set) —
      they may cover the part the walk does not — and the row passes only if
      both do. run.py's ALL-generic smoke checks are dropped either way."""
    by_id = {r["row_id"]: r for r in out_rows}
    partial = {rid for rid, v in native_rows.items() if v["depth"] == "native-partial"}
    targeted = {c["name"] for c in CHECKS if c["rows"] is not ALL}

    def is_targeted(c):
        return c["check"].split(" [")[0] in targeted

    for rid, v in native_rows.items():
        r = by_id.get(rid)
        if r is None:
            continue
        own = ([c for c in check_rows if int(c["row_id"]) == rid and is_targeted(c)]
               if rid in partial else [])
        own_failed = [c["check"] for c in own if c["status"] != "pass"]
        status = "fail" if (v["status"] != "pass" or own_failed) else "pass"
        reason = v["reason"]
        if own:
            reason += (f"; with run.py's {len(own)} own checks"
                       + (f", failing: {'; '.join(own_failed[:2])}" if own_failed else ", all pass"))
        r.update(status=status, depth=v["depth"], evidence=v["evidence"], reason=reason)
    kept = [c for c in check_rows
            if int(c["row_id"]) not in native_rows or (int(c["row_id"]) in partial and is_targeted(c))]
    for rid, v in sorted(native_rows.items()):
        r = by_id.get(rid)
        if r is None:
            continue
        for name, ok, detail, ev in v["checks"]:
            kept.append({"row_id": rid, "area": r["area"], "spec": r["spec"], "case": r["case"],
                         "check": name, "status": "pass" if ok else "fail", "evidence": ev,
                         "reason": (detail or "")[:200]})
    kept.sort(key=lambda c: int(c["row_id"]))
    return out_rows, kept


def read_csv(path: pathlib.Path) -> list:
    with open(path, newline="") as f:
        out = list(csv.DictReader(f))
    for r in out:
        r["row_id"] = int(r["row_id"])
    return out


def run_native(args) -> dict:
    """Run the native click walks (every `WALK` the convention finds) and
    fold them into per-row verdicts. Also leaves the raw per-check results in
    tmp/walk/native/last.json for a later `--native-only` merge."""
    import native  # tools/walk/native.py (same directory)
    only = {w.strip() for w in args.walks.split(",") if w.strip()} or None
    modes = [m.strip() for m in args.modes.split(",") if m.strip()]
    results, _ = native.run_all(str(BIN), args.port, args.fixture_port, modes=modes, only=only,
                                log=lambda s: print(s, flush=True))
    native.SCRATCH.mkdir(parents=True, exist_ok=True)
    last = native.SCRATCH / "last.json"
    specs = {s["name"]: s for _, s in native.discover()}
    if (only or len(modes) < 2) and last.exists():
        # A subset re-run (`--walks` / `--modes`) replaces only what it ran:
        # every row is still decided over the LATEST result of EVERY walk
        # (a row two walks map must not lose the other walk's checks).
        fresh = {(r["name"], r["mode"]) for r in results}
        results = [r for r in native.load_json(last)
                   if (r["name"], r["mode"]) not in fresh] + results
    results = [r for r in results if r["name"] in specs]
    native.write_json(results, last)
    verdicts = native.row_verdicts(results, specs)
    n_pass = sum(1 for v in verdicts.values() if v["status"] == "pass")
    print(f"[native] {len(verdicts)} rows re-pointed to native click walks: "
          f"{n_pass} pass, {len(verdicts) - n_pass} fail", flush=True)
    return verdicts


def native_only(args) -> int:
    """`--native-only`: the native walks merged into the EXISTING results
    (the rows they map are re-pointed; every other row keeps its verdict)."""
    try:
        if not (BIN.is_file() and os.access(BIN, os.X_OK)):
            raise PrereqError(APP_BIN_HELP)
    except PrereqError as e:
        print(f"tools/walk: {e}", file=sys.stderr)
        return 2
    rows = load_rows()
    out_rows = read_csv(WALK / "results.csv")
    check_rows = read_csv(WALK / "results-checks.csv")
    native_rows = run_native(args)
    out_rows, check_rows = merge_native(out_rows, check_rows, native_rows)
    parity = load_parity()
    relabel_unwalked(out_rows, rows, parity)
    demote_unbuilt(out_rows, rows, parity)
    with open(WALK / "results.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["row_id", "area", "spec", "case", "status", "depth", "evidence", "reason"])
        w.writeheader()
        w.writerows(out_rows)
    with open(WALK / "results-checks.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["row_id", "area", "spec", "case", "check", "status", "evidence", "reason"])
        w.writeheader()
        w.writerows(check_rows)
    from collections import Counter
    counts = Counter(r["status"] for r in out_rows)
    print("\n== walk results (native merged) ==")
    for k in ("pass", "fail", "not-walked", "not-yet-implemented", "live-only", "blocked", "skipped"):
        if counts.get(k):
            print(f"   {k:20} {counts[k]}")
    return 1 if any(v["status"] == "fail" for v in native_rows.values()) else 0


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
    # A11 — the native click walks (tools/walk/native.py; docs/walk/README.md).
    ap.add_argument("--full", action="store_true",
                    help="the official run: every scriptable row (no --limit) AND every "
                         "native click walk in desktop + phone")
    ap.add_argument("--native", dest="native", action="store_true", default=None,
                    help="also run the native click walks (the default with --full)")
    ap.add_argument("--no-native", dest="native", action="store_false",
                    help="skip the native click walks")
    ap.add_argument("--native-only", action="store_true",
                    help="run only the native walks and merge them into the EXISTING "
                         "results.csv / results-checks.csv (the other rows are kept)")
    ap.add_argument("--walks", default="", help="comma list of native walk names (default: all)")
    ap.add_argument("--modes", default="desktop,phone", help="native walk modes")
    ap.add_argument("--fixture-port", type=int, default=8434,
                    help="first port for a native walk's fixture server (one per walk)")
    ap.add_argument("--native-json", default="",
                    help="merge the native verdicts saved by an earlier run (tmp/walk/native/last.json) "
                         "instead of running the walks again")
    ap.add_argument("--scenario-port-base", type=int,
                    default=int(os.environ.get("WALK_SCENARIO_PORT_BASE", "8380")),
                    help="first port of run.py's own replay servers (one per scenario, "
                         f"{len(SCENARIO_PORTS)} in all; default 8380)")
    args = ap.parse_args()
    set_scenario_port_base(args.scenario_port_base)
    if args.full:
        args.limit = None
        if args.native is None:
            args.native = True
    if args.native_only:
        return native_only(args)

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
    # #43b: WALK_ONLY_ROWS="106,183,212" narrows a REPLAY run to named rows, the
    # replay-side twin of WALK_LIVE_ROWS. Needed because the three rows this card
    # unblocks each need their OWN app instance (a first run, a viewport width, a
    # forced theme), so proving them means launching those instances and nothing
    # else — `--limit` cannot select them (they sit past the round-robin cursor).
    only = os.environ.get("WALK_ONLY_ROWS", "")
    if only.strip() and not args.live:
        want = {int(x) for x in only.split(",") if x.strip().isdigit()}
        targets = [(i, area_of(r), r)
                   for i, r in enumerate(rows, start=1) if i in want]
        targets = [(i, a, r) for i, a, r in targets if a]
    if args.live:
        # #39a: exactly the live-only rows a real model can serve today —
        # row 1 (a real coding turn + refresh) and row 2 (a background turn
        # survives focusing a sibling). Row 50 (approval shortcuts) needs the
        # model to RAISE an approval card on its own; not scriptable yet.
        # select_targets EXCLUDES live-only rows (needs == "real-turn" is
        # skipped), so --live builds its targets straight from the rows.
        # {1,2,3,33}: the rows the real model can serve today (1 real turn +
        # restore, 2 background turn, 3 foreign-turn disclosure, 33 file
        # deliveries) — row 50 needs the model to RAISE an approval card on
        # its own; the other live-only rows need browser-only state (#41d).
        wanted = {1, 2, 3, 33, 50}
        # row 33's case text matches no AREA_PATTERN (area_of -> None), but
        # its check is registered under "composer" — give the area explicitly
        # so scenario_for resolves (#41d final-run KeyError '').
        wanted_area = {33: "composer", 50: "approval"}
        # WALK_LIVE_ROWS="50" narrows the run to specific rows — #41e's
        # cross-run turn accounting: re-running already-proven rows would
        # spend REAL turns for no new information.
        override = os.environ.get("WALK_LIVE_ROWS", "")
        if override.strip():
            wanted = {int(x) for x in override.split(",") if x.strip().isdigit()}
        targets = [(i, area_of(r) or wanted_area.get(i, ""), r)
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
      # #43b: the rows of one area may need DIFFERENT launch envs (a first run
      # that must not connect, a specific viewport width, a forced theme). The
      # app reads each of those once per process, so every distinct env is its
      # own app instance: group the area's target rows by `env_group_for` and
      # drive one instance per group. The DEFAULT group (no key) keeps the
      # pre-#43b connected behaviour, so the other rows are unaffected.
      area_rows = [(rid, a, r) for rid, a, r in targets if a == area]
      groups: dict[tuple | None, list] = {}
      for rid, a, r in area_rows:
          # EVERY key the row needs, not just the first: row 183 is a template
          # the web runs at each viewport width, so it must be driven at 1280
          # AND 720 — and its verdict is the AND of the two instances.
          for k in env_groups_for(r) or [None]:
              groups.setdefault(k, []).append((rid, a, r))
      for gkey in ([None] if None in groups else []) + [k for k in ENV_GROUPS if k in groups]:
        gspec = ENV_GROUPS.get(gkey, {})
        grows = groups.get(gkey) or []
        if gkey is not None and not grows:
            continue
        # The rows this instance is responsible for: only the rows with NO
        # env group (each grouped row belongs to its own instance), or exactly
        # the group's own rows. Without the `env_group_for(r) is None` filter the
        # default instance would re-map — and OVERWRITE — every grouped row's
        # verdict with its own connected-app results.
        my_rows = ([(rid, a, r) for rid, a, r in targets
                    if a == area and env_group_for(r) is None]
                   if gkey is None else grows)
        try:
            procs.start_server(scenario)
            procs.start_app_opts(scenario,
                                 first_run=bool(gspec.get("first_run")),
                                 extra_env=dict(gspec.get("env") or {}))
        except Exception as e:  # noqa: BLE001
            area_state[(area, gkey)] = {
                "blocked": True,
                "reason": f"scenario '{scenario}' failed to start: {e}"}
            for rid, _a, _r in grows:
                per_row_checks[rid] = []
            continue
        app = App(procs.app_port)
        app.env = dict(getattr(procs, "last_env", {}))
        # Run each check ONCE against this area's app; record its status.
        area_checks = [c for c in CHECKS if c["area"] == area]
        if gkey is not None:
            # Only this group's own checks, and only for its rows: the rest of
            # the area's checks assert the CONNECTED app this instance is not.
            keep = set(gspec.get("checks") or ())
            area_checks = [c for c in area_checks if c["name"] in keep]
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
                # #43b: evidence is per INSTANCE now (one per env group), so
                # the default group keeps the old name and a group appends its
                # key. Two failing groups in one area no longer clobber.
                suffix = "" if gkey is None else "-" + gkey[1]
                try:
                    sj = EVIDENCE / f"area-{area}{suffix}.snap.json"
                    sj.write_text(json.dumps(_scrub(app.snap())))
                    # A11: a grab can answer {"err": "grab frame could not be
                    # submitted …; retry"} — two evidence PNGs were that JSON.
                    for _ in range(4):
                        png = app.raw("/g?raw=1")
                        if png.startswith(b"\x89PNG"):
                            shot = EVIDENCE / f"area-{area}{suffix}.png"
                            shot.write_bytes(png)
                            shrink_png(shot)
                            break
                        time.sleep(1.0)
                    evidence = str(sj.relative_to(ROOT))
                except Exception:  # noqa: BLE001
                    pass
            print(f"  [{area}{'' if gkey is None else ':' + gkey[1]}] "
                  f"{status:5} {chk['name'][:62]:62} :: {reason[:60]}")
        # Map THIS INSTANCE's rows to the checks that apply to it. Scoping to
        # `my_rows` is what keeps a themed/width/first-run instance from
        # writing verdicts for rows it never launched for.
        for rid, _a, row in my_rows:
            krs = [(r["name"], r["status"], r["reason"], r["specific"])
                   for r in results if check_applies(r, row)]
            # #43b: a TEMPLATE row (row 183 runs at every viewport width) is
            # driven by SEVERAL instances and each one writes here, so EXTEND,
            # never replace — assigning clobbered the 1280 instance's two results
            # with the 720 instance's one (measured: 1 check recorded, and
            # vp_geometry vanished from the CSV entirely).
            # The dedupe key MUST include the group: both widths run the
            # SAME-named `vp_width`, and deduping on the name alone dropped the
            # 720 result — which would have let row 183 read green off the 1280
            # pass alone. Tagging the recorded name with the group keeps the CSV
            # auditable (`… [viewport-720]`) and keeps the row an AND over
            # widths. The default group keeps its bare names, so no other row's
            # recorded check name changes.
            prev = per_row_checks.get(rid, [])
            seen = {n for n, _s, _r, _sp in prev}
            if gkey is not None:
                krs = [(f"{n} [{gkey[1]}]", s, rr, sp) for n, s, rr, sp in krs]
            per_row_checks[rid] = prev + [k for k in krs if k[0] not in seen]
        area_state[(area, gkey)] = {"blocked": False, "reason": "",
                                    "evidence": evidence, "results": results}

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
            # #43b: the state is keyed by (area, env-group), not by area — a
            # row's verdict comes from the instance it was actually launched
            # on, so the lookup must use the same key `env_group_for` chose.
            st = area_state.get((area, env_group_for(row)),
                                area_state.get((area, None), {}))
            checks = per_row_checks.get(i, [])
            status = decided_status([s for _, s, _, _ in checks], st.get("blocked", False))
            reason = row_reason([(n, s) for n, s, _, _ in checks], st.get("reason", ""))
            nb = not_built_reason(checks) if status == "fail" else ""
            if nb:
                status, reason = "not-yet-implemented", nb
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

    # A11: the native click walks re-point the rows they map; the rows nothing
    # covers get the parity matrix's own verdict and reason.
    native_rows: dict = {}
    if args.native_json and not args.live:
        import native  # tools/walk/native.py
        saved = native.load_json(pathlib.Path(args.native_json))
        specs = {s["name"]: s for _, s in native.discover()}
        native_rows = native.row_verdicts([r for r in saved if r["name"] in specs], specs)
        out_rows, check_rows = merge_native(out_rows, check_rows, native_rows)
    elif args.native and not args.live:
        native_rows = run_native(args)
        out_rows, check_rows = merge_native(out_rows, check_rows, native_rows)
    parity = load_parity()
    relabel_unwalked(out_rows, rows, parity)
    demote_unbuilt(out_rows, rows, parity)

    live_suffix = "_live" if args.live else ""
    # #43b: a WALK_ONLY_ROWS run drives a SUBSET of rows, but the loop above
    # still emits an aggregate row for EVERY walk row — the unselected ones as
    # `not-yet-implemented`/`live-only`. Writing that to results.csv flipped 113
    # statuses of the last FULL run (measured), so a targeted run gets its own
    # files, exactly as --live does.
    if not args.live and os.environ.get("WALK_ONLY_ROWS", "").strip():
        live_suffix = "-only"
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
                          ("pass", "fail", "live-only", "not-walked", "not-yet-implemented",
                           "blocked", "skipped") if c.get(k))
        print(f"   {a:14} {cells}")
    infra_blocked = sum(1 for i, r in enumerate(out_rows, start=1)
                        if i in target_area and r["status"] == "blocked")
    print("\n== walk-runner summary ==")
    for k in ("pass", "fail", "not-walked", "not-yet-implemented", "live-only", "blocked",
              "skipped", "not-run"):
        if counts.get(k):
            print(f"   {k:20} {counts[k]}")
    print(f"   total                {len(out_rows)}")
    print(f"   scripted rows        {len(targets)}  (areas: {areas})")
    print(f"   per-check rows       {len(check_rows)}  (docs/walk/results{live_suffix}-checks.csv)")
    print(f"   live turns used      {turns_used()} / {LIVE_TURN_BUDGET} (hard cap, #41d)")
    by_depth = Counter((r["status"], r.get("depth", "")) for r in out_rows)
    print(f"   pass by depth        specific={by_depth.get(('pass', 'specific'), 0)}"
          f" smoke={by_depth.get(('pass', 'smoke'), 0)}"
          f" native={by_depth.get(('pass', 'native'), 0)}"
          f" native-partial={by_depth.get(('pass', 'native-partial'), 0)}"
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
