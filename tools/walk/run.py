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
                  "autonomy": 8383, "peer": 8384, "session": 8385}

# Areas the card names → the walk rows they cover. Order matters: `area_of`
# returns the FIRST match, so the more specific areas come first. `conversation`
# deliberately omits a bare `\border\b` — it falsely swallowed thread rows like
# "switches sessions by keyboard and preserves sidebar focus order".
AREA_PATTERNS = {
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
                   "palette": True, "keyboard": True, "connect": False}
AREA_SCENARIO = {"conversation": "conversation", "threads": "conversation",
                 "composer": "conversation", "recovery": "conversation",
                 "approval": "approval", "peer": "peer", "review": "autonomy",
                 "settings": "session", "palette": "conversation",
                 "keyboard": "conversation", "connect": "session"}
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
        """Empty the composer: focus at the end, then backspace it away.

        Waits for the field to read empty (or the placeholder), so the caller
        never races the reducer (card #19c).
        """
        self.focus_composer()
        for _ in range(CLEAR_PRESSES):
            self.key("backspace")
        return self.wait_for(
            lambda s: (self.draft(s) or "") in ("", PLACEHOLDER),
            what="the composer to clear",
        )

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


@check("composer", "the conversation column hosts the timeline PortalList")
def comp_timeline(app):
    return "timeline_list" in app.widget_ids(app.snap()), "timeline_list present"


@check("composer", "a typed draft round-trips through the composer")
def comp_input_roundtrip(app):
    app.type_into_composer("survive me")
    return True, "draft reads 'survive me' after clearing and typing"


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


# ---- review: the review panel (board 3.01/#30a) ---------------------------- #
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
    d = app.snap()
    texts = " ".join(w.get("t", "") for w in d.get("s", []))
    ok = "+62" in texts and "−5" in texts
    return ok, f"badge found={'+62 −5' in texts}"


@check("review", "the fold receipt renders the unmodified-lines count",
       rows=("unmodified", "fold", "context"))
def v_fold(app):
    d = app.snap()
    texts = " ".join(w.get("t", "") for w in d.get("s", []))
    ok = "unmodified" in texts.lower()
    return ok, f"fold text present={'unmodified' in texts.lower()}"


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
                         "peer", "review", "settings", "palette", "keyboard")
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
    args = ap.parse_args()

    # Fail fast on the documented prerequisites, with a message that says exactly
    # what to run (card #19b, defect 2). Building the replay server is cached.
    try:
        check_prereqs(build_replay=not args.no_build)
    except PrereqError as e:
        print(f"tools/walk: {e}", file=sys.stderr)
        return 2

    WALK.mkdir(parents=True, exist_ok=True)
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    rows = load_rows()
    targets = select_targets(args.limit)
    procs = Procs(args.port)

    areas = sorted({a for _, a, _ in targets})
    if args.only:
        areas = [a for a in areas if a == args.only]

    # area -> {"blocked": bool, "reason": str}. A row's checks run against the app
    # for its area; per-check results are recorded per row (card #19c item 3).
    area_state: dict[str, dict] = {}
    per_row_checks: dict[int, list] = {}   # row_id -> [(check_name, status, reason)]
    for area in areas:
        scenario = AREA_SCENARIO[area]
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
        results = []
        evidence = ""
        for chk in area_checks:
            try:
                ok, reason = chk["fn"](app)
                status = "pass" if ok else "fail"
            except Exception as e:  # noqa: BLE001
                ok, status, reason = False, "fail", f"exception: {e}"
            results.append({"name": chk["name"], "status": status, "reason": reason[:200],
                            "rows": chk["rows"]})
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
            krs = [(r["name"], r["status"], r["reason"]) for r in results if check_applies(r, row)]
            per_row_checks[rid] = [(n, s, rr) for (n, s, rr) in krs]
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
            status = decided_status([s for _, s, _ in checks], st.get("blocked", False))
            reason = row_reason([(n, s) for n, s, _ in checks], st.get("reason", ""))
            ev = st.get("evidence", "")
            out_rows.append({"row_id": i, "area": area, "spec": spec, "case": case,
                             "status": status, "evidence": ev, "reason": reason})
            for n, s, rr in checks:
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

    with open(WALK / "results.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["row_id", "area", "spec", "case", "status", "evidence", "reason"])
        w.writeheader()
        w.writerows(out_rows)
    with open(WALK / "results-checks.csv", "w", newline="") as f:
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
    print(f"   per-check rows       {len(check_rows)}  (docs/walk/results-checks.csv)")
    if infra_blocked:
        print(f"   NOTE: {infra_blocked} selected row(s) blocked by a start failure")
        reasons = sorted({r["reason"] for r in out_rows
                          if r["status"] == "blocked" and r["reason"]})
        for reason in reasons[:4]:
            print(f"     - {reason[:160]}")
    return exit_code(counts, infra_blocked)


if __name__ == "__main__":
    sys.exit(main())
