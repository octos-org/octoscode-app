#!/usr/bin/env python3
"""A35/A35b — the folder browser's latency, by CLICK, on the STANDALONE app.

    python3 tools/walk/a35_browser_latency_walk.py <desktop|phone> <outdir>
        [--bin target/release/octoscode] [--port 8470] [--fixture-port 8471]
        [--fixture-log <path>] [--budget-nav 100] [--budget-key 16]
        [--root /home/user/work] [--token-file <file> --profile <id>]

The operator (2026-10-03): "file browser is super slow — check why using the
makepad instrument, standalone mode, not OctoSense mode". This walk drives the
standalone desktop app (crates/octoscode-desktop, binary `octoscode`; default
the release build) and times every step from the input to the PIXELS: the
instrument's `/snap` reports a widget's rect only once a frame drew it, and
nothing else is sent while a step waits (no hover, no redraw nudge), so a
listing that lands in the widget tree but is never drawn FAILS.

The server: board1_serve's realistic tree by default (`--tree realistic`: a
46-project working folder with 5 hidden dot-folders, a 150-package monorepo,
a 650-folder node_modules the server pages at 500); it answers in well under
a millisecond, so every millisecond measured is the client's. With
`--token-file`, the walk drives a REAL `octos serve` already listening on
`--fixture-port` whose working directory holds the same tree under `--root`
(the token reaches the app only through its environment; request counts are
then the fixture's to prove and are skipped).

Bars (absolute; the integrator's, A35b):
- a navigation — the click (or Return) to the new listing DRAWN: < --budget-nav ms;
- a key in the path box, and a wheel step over a long listing — the input to
  its drawn frame (`wait=1`): < --budget-key ms.
Request counts come from the fixture's own log (one line per request):
+ Add workspace lists the server's folder ONCE, a pick or a key sends
nothing, a navigation sends one. Remounts come from the app log
(`[octoscode] board1 remounted`): none per key.

Instrument note: the first `/snap` after a remount rebuilds the widget
tree's dense index, which clears its path cache; the app's next event then
re-walks the tree (~25 ms in a release build). The real app never pays this
(nothing but the instrument asks for that index), so these numbers are an
upper bound on what a person sees.

Writes <outdir>/timings.json, <mode>-*.png captures, walk.log (the board-1,
perf and ui-hang lines). Prints PASS/FAIL per check; exit 0 iff all pass.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)
from walk_env import ROOT, isolated_env, scrub  # noqa: E402

# The aggregator's convention (tools/walk/native.py): read with `ast`, never
# imported. The walk launches its own STANDALONE app ("self") against the
# realistic fixture the aggregator starts.
WALK = {
    "name": "a35_browser_latency",
    "title": "the folder browser on the standalone app: every navigation drawn within 100 ms, a key and a wheel step within 16 ms, by clicks",
    "modes": ["desktop", "phone"],
    "app": "self",
    "fixture": {"argv": ["{examples}/board1_serve", "{fport}", "--tree", "realistic"]},
    "runs": [{"argv": ["{mode}", "{out}", "--port", "{port}", "--fixture-port", "{fport}",
                       "--fixture-log", "{fixture_log}"]}],
    "needs": ["target/debug/examples/board1_serve", "target/release/octoscode"],
    "timeout": 600,
}

ap = argparse.ArgumentParser()
ap.add_argument("mode", choices=["desktop", "phone"])
ap.add_argument("out")
ap.add_argument("--bin", default=None)
ap.add_argument("--port", type=int, default=8470)
ap.add_argument("--fixture-port", type=int, default=8471)
ap.add_argument("--fixture-log", default="")
ap.add_argument("--budget-nav", type=float, default=100.0)
ap.add_argument("--budget-key", type=float, default=16.0)
ap.add_argument("--root", default="/home/user/work", help="the server's working directory")
ap.add_argument("--token-file", default="", help="a REAL serve: its token, read here, never printed")
ap.add_argument("--profile", default="octoscode")
ap.add_argument("--hang-ms", default="50")
ARGS = ap.parse_args()

OUT = pathlib.Path(ARGS.out)
OUT.mkdir(parents=True, exist_ok=True)
BASE = f"http://127.0.0.1:{ARGS.port}"
PHONE = ARGS.mode == "phone"
LIVE = bool(ARGS.token_file)
WORK = ARGS.root.rstrip("/")
PROJECTS = sorted([
    "api-gateway", "auth-service", "billing", "blog", "cli-tools", "data-pipeline", "deploy", "design-system",
    "docs-site", "dotfiles", "experiments", "frontend", "game-jam", "homelab", "infra", "ios-app", "kernel-notes",
    "landing-page", "ml-models", "mobile", "monorepo", "notebooks", "octos", "octoscode-app", "ops-scripts",
    "payments", "photo-tools", "playground", "plugins", "portfolio", "prototype", "research", "rust-learning",
    "scraper", "search", "sensors", "shaders", "snippets", "static-assets", "terraform", "themes", "tui-app",
    "vendor", "web-app", "wiki", "zig-play",
], key=str.lower)
USUAL = ["assets", "docs", "scripts", "src", "tests"]
MONOREPO = ["apps", "docs", "packages", "scripts", "tools"]
HANG = re.compile(r"\[ui-hang\] (\d+) ms · phase=(\S+)")
RESULTS: list = []
TIMINGS: list = []


def report(name: str, ok: bool, detail: str = "") -> bool:
    RESULTS.append((name, bool(ok)))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
    return bool(ok)


def binary() -> pathlib.Path:
    if ARGS.bin:
        return pathlib.Path(ARGS.bin).resolve()
    for profile in ("release", "debug"):
        p = ROOT / "target" / profile / "octoscode"
        if p.exists():
            return p
    raise SystemExit("no target/{release,debug}/octoscode: cargo build --release -p octoscode-desktop")


def profile_of(exe: pathlib.Path) -> str:
    """target/<profile>/octoscode -> <profile>; another path -> its file name."""
    return exe.parent.name if exe.name == "octoscode" else exe.name


# ------------------------------------------------------------ the fixture
class Fixture:
    """board1_serve --tree realistic, unless one (or a real serve) already listens."""

    def __init__(self):
        self.proc = None
        self.log = pathlib.Path(ARGS.fixture_log) if ARGS.fixture_log else OUT / "fixture.log"

    def up(self) -> bool:
        try:
            socket.create_connection(("127.0.0.1", ARGS.fixture_port), timeout=0.3).close()
            return True
        except OSError:
            return False

    def start(self):
        if self.up():
            return
        exe = ROOT / "target" / "debug" / "examples" / "board1_serve"
        if not exe.exists():
            raise SystemExit("cargo build -p octoscode-module --example board1_serve")
        self.proc = subprocess.Popen([str(exe), str(ARGS.fixture_port), "--tree", "realistic"],
                                     stdout=open(self.log, "w"), stderr=subprocess.STDOUT)
        end = time.time() + 20
        while time.time() < end and not self.up():
            time.sleep(0.1)

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(5)

    def lists(self) -> int | None:
        """`onboarding/workspace_list` requests the fixture has received (None: a real serve)."""
        if LIVE:
            return None
        try:
            return self.log.read_text(errors="ignore").count("<- onboarding/workspace_list")
        except OSError:
            return None


# ---------------------------------------------------------------- the app
STATE = OUT / "state"


def app_env() -> dict:
    env = dict(os.environ)
    for k in ("OCTOS_BEARER", "OCTOS_PAIRING_LINK", "OCTOSENSE_WINDOW_SIZE", "OCTOSCODE_DESIGN_DIR",
              "MAKEPAD_WM_TEST_APP", "HEADLESS_ARGS", "OCTOSCODE_SCREEN"):
        env.pop(k, None)
    # Brief §8: isolated state; HOME too (the standalone app materializes its
    # design cache under $HOME/.octoscode).
    env.update(isolated_env(STATE))
    (STATE / "home").mkdir(parents=True, exist_ok=True)
    token = pathlib.Path(ARGS.token_file).read_text().strip() if LIVE else "walk-dummy-token"
    env.update({
        "HOME": str(STATE / "home"),
        "HEADLESS_STATE": str(OUT / "hs"),
        "HEADLESS_TIMEOUT": "90",
        "OCTOS_BASE_URL": f"http://127.0.0.1:{ARGS.fixture_port}",
        "OCTOS_BEARER": token,
        "OCTOS_PROFILE_ID": ARGS.profile,
        "OCTOS_WORKSPACE_CWD": WORK,
        # The UI-stall sampler names any event over this in the app log; the
        # module's own probe names each event's widget-tree cost.
        "MAKEPAD_UI_HANG_MS": ARGS.hang_ms,
        "OCTOSCODE_PERF": "1",
    })
    if PHONE:
        env["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    return env


def get(path: str, timeout: float = 30) -> bytes:
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read()
    except urllib.error.HTTPError as e:
        if bridgeauth.input_was_queued(path, e):
            return b""
        raise


def snap(q: str, drawn_only: bool = True) -> list:
    qs = "q=" + urllib.parse.quote(q) + ("" if drawn_only else "&all=1")
    return json.loads(get("/snap?" + qs)).get("s", [])


def drawn(w: dict) -> bool:
    r = w.get("r") or [0, 0, 0, 0]
    return r[2] > 0 and r[3] > 0 and w.get("v", 1) != 0


def find(wid: str):
    for w in snap(wid):
        if w.get("i") == wid and drawn(w):
            return w
    return None


def text(wid: str):
    w = find(wid)
    return w.get("t") if w else None


def wait_for(pred, secs: float, period: float = 0.25):
    end = time.time() + secs
    while time.time() < end:
        try:
            v = pred()
            if v:
                return v
        except (urllib.error.URLError, OSError, ValueError):
            pass
        time.sleep(period)
    return None


class Stop(Exception):
    """A control the next step needs was never drawn: the walk cannot go on."""


RECTS: dict = {}


def click(wid: str, as_name: str | None = None) -> None:
    w = find(wid)
    if not w:
        raise Stop(f"no drawn {wid} to click")
    x, y, ww, hh = w["r"]
    # Where it was, for the unpolled pass (which sends no /snap at all).
    RECTS.setdefault(as_name or wid, w["r"])
    get(f"/click?x={x + ww / 2:.1f}&y={y + hh / 2:.1f}&wait=1")


def key(code: str, **mods) -> None:
    extra = "".join(f"&{m}=1" for m, on in mods.items() if on)
    get(f"/k?k=down&c={code}{extra}&wait=1")
    get(f"/k?k=up&c={code}{extra}&wait=1")


def type_text(t: str) -> None:
    get("/t?" + urllib.parse.urlencode({"t": t, "wait": 1}))


LOG_SEQ = [0]


def app_log() -> list:
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}" if LOG_SEQ[0] else "/log?n=20000"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return d.get("l", [])


def remounts(lines: list) -> int:
    return sum(1 for l in lines if "[octoscode] board1 remounted" in l)


def hangs(lines: list) -> list:
    out = []
    for l in lines:
        m = HANG.search(l)
        if m:
            out.append((m.group(2), int(m.group(1))))
    return out


def timed(name: str, action, wid: str, want: str, budget: float, fx: Fixture, lists: int | None = None,
          secs: float = 3.0) -> float | None:
    """One step: the input, then poll (nothing else sent) until `wid` is DRAWN
    showing `want`. `lists`: the `workspace_list` requests the step must send."""
    app_log()
    before = fx.lists()
    t0 = time.perf_counter()
    action()
    ack = time.perf_counter()
    seen = None
    while time.perf_counter() < t0 + secs:
        for w in snap(wid):
            if w.get("i") == wid and drawn(w) and w.get("t") == want:
                seen = time.perf_counter()
                break
        if seen:
            break
        time.sleep(0.002)
    ms = round((seen - t0) * 1000, 1) if seen else None
    rec = {"step": name, "ms": ms, "input_ack_ms": round((ack - t0) * 1000, 1), "budget_ms": budget}
    if ms is None:
        rec["in_widget_tree_but_not_drawn"] = any(
            w.get("i") == wid and w.get("t") == want for w in snap(wid, drawn_only=False))
    time.sleep(0.3)
    lines = app_log()
    rec["remounts"] = remounts(lines)
    rec["ui_hangs"] = hangs(lines)
    after = fx.lists()
    if lists is not None and before is not None and after is not None:
        rec["workspace_list_requests"] = after - before
    TIMINGS.append(rec)
    detail = (f"{ms} ms (bar {budget:g} ms)" if ms is not None else
              f"not drawn within {secs:g} s" + (" — the listing is in the widget tree but no frame drew it"
                                                 if rec.get("in_widget_tree_but_not_drawn") else ""))
    report(f"{name}: drawn within {budget:g} ms", ms is not None and ms < budget, detail)
    if "workspace_list_requests" in rec:
        report(f"{name}: {lists} workspace_list request(s)", rec["workspace_list_requests"] == lists,
               f"{rec['workspace_list_requests']} sent")
    return ms


def capture(name: str) -> pathlib.Path:
    png = OUT / f"{ARGS.mode}-{name}.png"
    png.write_bytes(get("/g?raw=1", timeout=60))
    subprocess.run(["sips", "-Z", "1400", str(png), "--out", str(png)], capture_output=True)
    return png


def rows_drawn(n: int) -> list:
    got = {}
    for w in snap("b1_br_row_t"):
        i = w.get("i", "")
        if i.startswith("b1_br_row_t") and drawn(w):
            got[int(i[len("b1_br_row_t"):])] = w.get("t")
    return [got.get(i) for i in range(n)]


def layout_checks(state: str) -> str:
    """The brief's numeric /snap checks of one browser state: the card inside
    the window, no row text outside its row, no control under 28 px, no two
    rows overlapping. Returns the checks string for docs/ux-scores.csv."""
    s = json.loads(get("/snap?all=1")).get("s", [])
    shown = [w for w in s if drawn(w)]
    win = json.loads(get("/s"))["w"][0]["sz"]
    card = next((w["r"] for w in shown if w.get("i") == "b1_card"), None)
    sheet = next((w["r"] for w in shown if w.get("i") == "b1_sheet"), None)
    frame = card or sheet
    rows = sorted((w for w in shown if re.fullmatch(r"b1_br_row_\d+", w.get("i", ""))), key=lambda w: w["r"][1])
    lst = next((w["r"] for w in shown if w.get("i") == "b1_br_list"), None)
    texts = {w["i"]: w["r"] for w in shown if re.fullmatch(r"b1_br_row_t\d+", w.get("i", ""))}
    control = re.compile(r"b1_br_(row_\d+|crumb_\d+|path|use|back|backto|newfolder)")

    def clipped(r):
        # A row the list scrolled half out reports its clipped rect.
        return lst is not None and (r[1] <= lst[1] + 1 or r[1] + r[3] >= lst[1] + lst[3] - 1)

    small = [w["i"] for w in shown if control.fullmatch(w.get("i", "")) and w["r"][3] < 28
             and not (w["i"].startswith("b1_br_row_") and clipped(w["r"]))]
    outside = []
    for i, r in texts.items():
        row = next((w["r"] for w in rows if w["i"] == "b1_br_row_" + i[len("b1_br_row_t"):]), None)
        if row and (r[0] < row[0] or r[0] + r[2] > row[0] + row[2] + 1):
            outside.append(i)
    overlaps = sum(1 for a, b in zip(rows, rows[1:]) if a["r"][1] + a["r"][3] > b["r"][1] + 1)
    inside = frame is not None and frame[0] >= 0 and frame[1] >= 0 and frame[0] + frame[2] <= win[0] + 1 \
        and frame[1] + frame[3] <= win[1] + 1
    margins = f"l/r [{frame[0]}, {win[0] - frame[0] - frame[2]}]" if frame else "no card"
    out = (f"{state}: window {win}; card {frame} {margins}; inside={inside}; rows drawn {len(rows)}; "
           f"row text outside its row {len(outside)}; controls<28px {len(small)}; row overlaps {overlaps}")
    report(f"layout ({state}): the card inside the window, rows clean", inside and not outside and not small
           and not overlaps, out)
    return out


def main() -> int:
    exe = binary()
    print(f"[a35] standalone {exe.relative_to(ROOT) if exe.is_relative_to(ROOT) else exe.name} "
          f"({profile_of(exe)}), {ARGS.mode}{', live serve' if LIVE else ''}", flush=True)
    fx = Fixture()
    if not LIVE:
        fx.start()
    env = app_env()
    started = False
    try:
        r = subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "start", str(exe), str(ARGS.port)],
                           cwd=str(ROOT), env=env, capture_output=True, text=True)
        started = r.returncode == 0
        listening = next((l for l in r.stdout.splitlines() if "listening on" in l), r.stdout[-160:])
        if not report("the standalone app starts hidden with its bridge", started, scrub(listening.strip())):
            return 1
        return walk(fx)
    except Stop as e:
        report("the walk reached its last step", False, f"stopped: {e}")
        return 1
    finally:
        try:
            lines = [l for l in json.loads(get("/log?n=20000")).get("l", [])
                     if "[octoscode] board1" in l or "[ui-hang]" in l or "[octoscode] perf:" in l]
            (OUT / "walk.log").write_text(scrub("\n".join(l.split(" - ", 1)[-1][:400] for l in lines)) + "\n")
        except Exception:  # noqa: BLE001
            pass
        if started:
            subprocess.run(["bash", str(ROOT / "harness/headless.sh"), "stop", str(ARGS.port)], cwd=str(ROOT),
                           env=dict(os.environ, HEADLESS_STATE=str(OUT / "hs")), capture_output=True)
        fx.stop()
        summary = {"binary_profile": profile_of(exe), "mode": ARGS.mode, "live": LIVE, "steps": TIMINGS,
                   "passed": sum(1 for _, ok in RESULTS if ok), "checks": len(RESULTS)}
        (OUT / "timings.json").write_text(scrub(json.dumps(summary, indent=1)) + "\n")
        print(f"== {summary['passed']}/{summary['checks']} checks passed ({ARGS.mode}, {summary['binary_profile']})",
              flush=True)


PERF = re.compile(r"\[octoscode\] perf: (\w+) ([\d.]+) ms @([\d.]+)")


def app_clock(lines: list) -> float | None:
    """The step's latency on the app's clock: from the start of its first
    event to the end of the first Draw after its LAST board-1 remount."""
    events, last_remount = [], None
    for l in lines:
        m = PERF.search(l)
        if m:
            events.append((m.group(1), float(m.group(2)), float(m.group(3))))
        elif "[octoscode] board1 remounted" in l:
            last_remount = len(events)
    if not events or last_remount is None:
        return None
    start = events[0][2] - events[0][1]
    draw = next((e for e in events[last_remount:] if e[0] == "Draw"), None)
    return round(draw[2] - start, 1) if draw else None


def unpolled(fx: Fixture) -> None:
    nav = ARGS.budget_nav

    def at(name: str) -> str:
        x, y, w, h = RECTS[name]
        return f"/click?x={x + w / 2:.1f}&y={y + h / 2:.1f}&wait=1"

    def step(name: str, inputs: list, lists: int | None) -> None:
        app_log()
        before = fx.lists()
        for path in inputs:
            get(path)
        time.sleep(0.8)
        lines = app_log()
        ms = app_clock(lines)
        after = fx.lists()
        rec = {"step": f"unpolled {name}", "app_clock_ms": ms, "budget_ms": nav, "remounts": remounts(lines),
               "ui_hangs": hangs(lines)}
        if lists is not None and before is not None and after is not None:
            rec["workspace_list_requests"] = after - before
        TIMINGS.append(rec)
        report(f"unpolled {name}: drawn within {nav:g} ms on the app's clock", ms is not None and ms < nav,
               f"{ms} ms, {rec['remounts']} remount(s)")

    # Back at the server's folder (the last polled step). A throwaway pick
    # first: it absorbs the cache the last /snap cleared.
    get(at("b1_br_row_0"))
    time.sleep(0.6)
    step("pick: a first tap", [at("b1_br_row_2")], 0)
    step("drill: a second tap opens the folder", [at("b1_br_row_2")], 1)
    step("up: the breadcrumb opens the parent", [at("crumb_parent")], 1)
    # Escape twice closes the browser and the picker; then + Add workspace.
    for _ in range(2):
        get("/k?k=down&c=Escape&wait=1")
        get("/k?k=up&c=Escape&wait=1")
        time.sleep(0.4)
    if PHONE:
        get(at("sidebar_toggle_hit"))
        time.sleep(0.6)
    step("open: + Add workspace -> the listing", [at("sb_add_hit")], 1)


def walk(fx: Fixture) -> int:
    entry = "sidebar_toggle_hit" if PHONE else "sb_add_hit"
    if not report("connected: the workspace entry is drawn", wait_for(lambda: find(entry), 90, 0.5)):
        return 1
    time.sleep(2.0)
    app_log()
    if PHONE and not find("sb_add_hit"):
        click("sidebar_toggle_hit")
        wait_for(lambda: find("sb_add_hit"), 10)
        time.sleep(0.6)
    nav, k = ARGS.budget_nav, ARGS.budget_key
    layout = {}

    # 1. + Add workspace -> the server's folder, listed ONCE, drawn.
    timed("open: + Add workspace -> the listing", lambda: click("sb_add_hit"), "b1_br_row_t0", PROJECTS[0],
          nav, fx, lists=1)
    shown = rows_drawn(3)
    report("open: the rows are the server's folders in order", shown == PROJECTS[:3], f"{shown}")
    for i in range(3):
        row = find(f"b1_br_row_{i}")
        if row:
            RECTS[f"b1_br_row_{i}"] = row["r"]
    note = text("b1_br_notice") or ""
    report("open: the server's hidden folders are reported", "5 hidden by the server" in note, repr(note))
    layout["open"] = layout_checks("open")
    capture("open")
    # 2. a first tap picks: the path box fills, nothing is requested.
    timed("pick: a first tap fills the path box", lambda: click("b1_br_row_2"), "b1_br_path",
          f"{WORK}/{PROJECTS[2]}", nav, fx, lists=0)
    # 3. a second tap opens it.
    timed("drill: a second tap opens the folder", lambda: click("b1_br_row_2"), "b1_br_row_t0", USUAL[0],
          nav, fx, lists=1)
    report("drill: the folder's rows", rows_drawn(5) == USUAL, f"{rows_drawn(5)}")
    capture("drilled")
    # 4. Up: the breadcrumb's parent.
    crumbs = sorted((w for w in snap("b1_br_crumb_") if w.get("i", "").startswith("b1_br_crumb_") and drawn(w)),
                    key=lambda w: w["r"][0])
    parent = WORK.rsplit("/", 1)[-1]
    up = next((w["i"] for w in crumbs if w.get("t") == parent), None)
    if report("up: the breadcrumb names the parent", up is not None, f"{[w.get('t') for w in crumbs]}"):
        timed("up: the breadcrumb opens the parent", lambda: click(up, "crumb_parent"), "b1_br_row_t0",
              PROJECTS[0], nav, fx, lists=1)
    # 5. typing: every key's frame within the bar, no request, no remount.
    click("b1_br_path")
    time.sleep(0.4)
    key("End")
    time.sleep(0.2)
    app_log()
    before = fx.lists()
    per_key = []
    for ch in "/monorepo":
        t0 = time.perf_counter()
        type_text(ch)
        per_key.append(round((time.perf_counter() - t0) * 1000, 1))
    time.sleep(0.3)
    box = text("b1_br_path")
    lines = app_log()
    sent = (fx.lists() - before) if before is not None else None
    TIMINGS.append({"step": "type '/monorepo' in the path box", "per_key_ms": per_key, "budget_ms": k,
                    "workspace_list_requests": sent, "remounts": remounts(lines), "ui_hangs": hangs(lines)})
    report(f"type: every key's frame within {k:g} ms", max(per_key) < k, f"max {max(per_key)} ms, keys {per_key}")
    report("type: the path box shows the typed path", box == f"{WORK}/monorepo", repr(box))
    report("type: no remount per key", remounts(lines) == 0, f"{remounts(lines)} remounts")
    if sent is not None:
        report("type: no request per key", sent == 0, f"{sent} requests")
    # 6. Return opens the typed path.
    timed("go: Return opens the typed folder", lambda: key("ReturnKey"), "b1_br_row_t0", MONOREPO[0], nav, fx,
          lists=1)
    # 7. a 150-package listing: pick, open, scroll.
    timed("pick: packages", lambda: click("b1_br_row_2"), "b1_br_path", f"{WORK}/monorepo/packages", nav, fx,
          lists=0)
    timed("drill: packages (150 folders)", lambda: click("b1_br_row_2"), "b1_br_row_t0", "pkg-000", nav, fx,
          lists=1)
    lst = find("b1_br_list")
    if report("scroll: the long listing scrolls inside the card", lst is not None):
        x, y, w, h = lst["r"]
        app_log()
        per_step = []
        for _ in range(6):
            t0 = time.perf_counter()
            get(f"/m?k=scroll&x={x + w / 2:.0f}&y={y + h / 2:.0f}&dy=120&wait=1")
            per_step.append(round((time.perf_counter() - t0) * 1000, 1))
        time.sleep(0.3)
        lines = app_log()
        TIMINGS.append({"step": "scroll the 150-folder listing (6 wheel steps)", "per_step_ms": per_step,
                        "budget_ms": k, "remounts": remounts(lines), "ui_hangs": hangs(lines)})
        report(f"scroll: every wheel step's frame within {k:g} ms", max(per_step) < k,
               f"max {max(per_step)} ms, steps {per_step}")
        top = sorted((wd for wd in snap("b1_br_row_t") if drawn(wd) and wd["r"][1] >= y), key=lambda wd: wd["r"][1])
        report("scroll: the list moved, no remount", bool(top) and top[0].get("t") != "pkg-000" and remounts(lines) == 0,
               f"top row now {top[0].get('t') if top else None}, {remounts(lines)} remounts")
        layout["packages, scrolled"] = layout_checks("packages, scrolled")
        capture("packages")
    # 8. 650 folders: the server's page of 500, truncated and said so.
    click("b1_br_path")
    time.sleep(0.3)
    key("KeyA", cmd=True)
    type_text(f"{WORK}/frontend/node_modules")
    time.sleep(0.3)
    timed("go: node_modules (650 folders, the server pages 500)", lambda: key("ReturnKey"), "b1_br_row_t0",
          "dep-000", nav, fx, lists=1)
    note = text("b1_br_notice") or ""
    report("node_modules: the truncation is reported", "Only the first 500 folders are shown." in note, repr(note))
    layout["node_modules"] = layout_checks("node_modules")
    capture("node-modules")
    # 9. a refused folder -> p4-09, then back without a request.
    click("b1_br_path")
    time.sleep(0.3)
    key("KeyA", cmd=True)
    type_text("/private")
    time.sleep(0.3)
    timed("refused: /private -> the bounded refusal", lambda: key("ReturnKey"), "b1_br_backto",
          f"Back to {WORK}/frontend/node_modules", nav, fx, lists=1)
    layout["refused"] = layout_checks("refused")
    capture("refused")
    timed("back: Back to the last good folder", lambda: click("b1_br_backto"), "b1_br_row_t0", "dep-000", nav, fx,
          lists=0)
    # 10. the back chevron -> the picker; its Browse reopens at the server's folder.
    timed("close: the back chevron -> the picker", lambda: click("b1_br_back"), "b1_pk_browse",
          "Browse folders…", nav, fx, lists=0)
    timed("browse: the picker's Browse -> the server's folder", lambda: click("b1_pk_browse"), "b1_br_row_t0",
          PROJECTS[0], nav, fx, lists=1)
    # 11. The same navigations UNPOLLED: no /snap between the inputs (the
    # instrument's dense-index rebuild clears the widget tree's path cache),
    # read off the app's own clock: from the input's event to the end of
    # the draw that showed the new listing (OCTOSCODE_PERF `@` times).
    unpolled(fx)
    # The whole walk: no UI-thread event ran long.
    long = [h for rec in TIMINGS for h in rec.get("ui_hangs", []) if h[1] >= nav]
    report(f"no UI-thread event of {nav:g} ms or more during the browser steps", not long, f"{long}")
    (OUT / "layout.json").write_text(json.dumps(layout, indent=1) + "\n")
    return 0 if all(ok for _, ok in RESULTS) else 1


if __name__ == "__main__":
    sys.exit(main())
