#!/usr/bin/env python3
"""A12 — the outage walk: a LIVE connection whose server is killed and comes
back, observed and driven through the real UI (CLICKs at /snap rects).

The walk owns both processes: it starts the server (the replay fixture, or a
real `octos serve` instance for the live proof), launches the hidden app
against it with every app store in a fresh temp dir (brief §8, never
~/.octoscode), kills the server, restarts it, and asserts what the window
shows at each step:

  1. live: the shell (sidebar + conversation + composer), strip "Ready";
     [live] one real turn streams; an unsent draft is typed.
  2. the server dies (SIGTERM, or SIGKILL with A12_KILL=KILL): the SHELL
     STAYS (no Connect card), the banner reads "Reconnecting to Octos" over
     "Connection lost · retry N · <server in use>", the strip says
     "Reconnecting", the draft is intact; numeric layout checks; a CLICK on
     Send is refused on the banner ("Not sent — your text stays in the
     composer."), nothing reaches the wire; a CLICK on Retry now re-dials.
  3. the server returns on the SAME port: the banner goes, the strip is
     "Ready", the trace shows the SAME Session re-opened ({reconnect: true})
     and hydrated, the draft survived; [live] the draft is sent as the next
     prompt and streams.
     (A12_NEW_PORT=<p>: it returns on ANOTHER port instead — the banner keeps
     naming the old server, never a default; Disconnect -> the Connect card
     shows the old address; the user re-points it to the new port; the old
     conversation is there.)
  4. explicit give-up: the server dies again; [replay] a typed "/review" +
     Return is refused on the banner with the text kept; a CLICK on the
     banner's Disconnect (+ A9's confirmation) returns the Connect card ON
     THE SERVER IN USE; the server returns and the card's Connect dials it.

usage:
  A12_HOSTBIN=<host octosense> python3 tools/walk/a12_reconnect_walk.py <app-port> <desktop|phone> <shots-dir>
env:
  A12_SERVER=replay|live|none  (default replay: target/debug/examples/replay_serve --scenario activity;
                           none: the transport-less shell, a typed command / a send with NO conversation)
  A12_SERVE_PORT           (default 8437; the walk checks its OWN server holds it)
  A12_KILL=TERM|KILL       (default TERM)
  A12_NEW_PORT=<port>      (the port-change variant)
  A12_DOWN_S=<seconds>     (stay down this long before the restart; default 7)
  A12_RETRY=during|after|none  (click Retry now while down / right after the
                           restart / never — pure self-recovery; default during)
  live only: A12_OCTOS_BIN, A12_LIVE_DIR (holds data/ inst/ ws/ and a mode-600
  `token` file — never printed), A12_PROFILE (default dsflash), A12_TURNS=1
Exit status 0 when every check passes.
"""
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request

WS = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8422
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
SHOTS = sys.argv[3] if len(sys.argv) > 3 else None
SERVER = os.environ.get("A12_SERVER", "replay")
SPORT = int(os.environ.get("A12_SERVE_PORT", "8437"))
NEW_PORT = int(os.environ["A12_NEW_PORT"]) if os.environ.get("A12_NEW_PORT") else None
KILL = signal.SIGKILL if os.environ.get("A12_KILL", "TERM").upper() == "KILL" else signal.SIGTERM
DOWN_S = float(os.environ.get("A12_DOWN_S", "7"))
# Retry now: "during" the outage (re-dials while down), "after" the restart
# (skips the transport's backoff), or "none" (pure self-recovery).
RETRY = os.environ.get("A12_RETRY", "during")
TURNS = SERVER == "live" and os.environ.get("A12_TURNS", "1") == "1"
PROFILE = os.environ.get("A12_PROFILE", "dsflash" if SERVER == "live" else "a12walk")
HOSTBIN = os.environ.get("A12_HOSTBIN", "")
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []
STATE = tempfile.mkdtemp(prefix="a12walk.")
TRACE = os.path.join(STATE, "trace.jsonl")
SERVE_LOG = os.path.join(STATE, "serve.log")
DRAFT = ("In one short sentence: what does main.rs in this workspace print?" if SERVER == "live"
         else "unsent words survive the outage")


# ---------------------------------------------------------------- instrument
def get(path, timeout=20):
    action = path.startswith(("/t?", "/click", "/k?"))
    for attempt in range(6):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except Exception:
            if action:
                time.sleep(0.4)
                return b""
            if attempt == 5:
                raise
            time.sleep(0.4)


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(s, wid):
    return [w for w in s if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def shown(wid, s=None):
    return bool(visible(s if s is not None else snap(), wid))


def rect(wid, s=None):
    hits = sorted(visible(s if s is not None else snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[0]["r"] if hits else None


def text(wid, s=None):
    hits = visible(s if s is not None else snap(), wid)
    return hits[0].get("t", "") if hits else None


def click_rect(r):
    x, y, w, h = r
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.3)


def click(wid):
    r = rect(wid)
    if r is None:
        return False
    click_rect(r)
    return True


def type_text(t):
    get("/t?" + urllib.parse.urlencode({"t": t, "wait": 1}))
    time.sleep(0.3)


def key(code):
    get(f"/k?c={code}&wait=1")
    time.sleep(0.2)


LOG_SEQ = [0]


def log_since():
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return [l if isinstance(l, str) else json.dumps(l) for l in d.get("l", [])]


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)


def soon(pred, secs=10.0, pause=0.3):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(pause)
    try:
        return bool(pred())
    except Exception:
        return False


def shot(name):
    if not SHOTS:
        return
    os.makedirs(SHOTS, exist_ok=True)
    raw = os.path.join(SHOTS, f".{name}.raw.png")
    out = os.path.join(SHOTS, f"{name}.png")
    with open(raw, "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", raw, "--out", out], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.remove(raw)
    keep = ("a12_", "b3_strip", "connect_", "hd_title", "a9_lv_")
    keep_ids = ("composer_row", "strip_row", "conversation_column", "sidebar_dock", "i0_composer_0", "link_row")
    rows = [dict(w) for w in snap() if w.get("i", "").startswith(keep) or w.get("i") in keep_ids]
    for w in rows:
        # The instrument reports a masked field's RAW value in `val` (the
        # access token): never written anywhere.
        if "token" in (w.get("i") or "") or set((w.get("t") or "").strip()) == {"•"}:
            w.pop("val", None)
    with open(os.path.join(SHOTS, f"{name}.snap.json"), "w") as f:
        json.dump(rows, f)
    print(f"  shot {out}", flush=True)


def composer_text(s=None):
    """The composer's value (`val`; `t` on older instruments, where the
    empty field reports its placeholder)."""
    v = visible(s if s is not None else snap(), "i0_composer_0")
    if not v:
        return ""
    val = v[0].get("val")
    if val is not None:
        return val
    t = v[0].get("t") or ""
    return "" if t == "Ask Octos anything" else t


def clear_composer():
    n = len(composer_text())
    get("/k?c=end&wait=1")
    for _ in range(max(n + 1, 4)):
        get("/k?c=backspace&wait=1")
    time.sleep(0.3)


def trace():
    out = []
    try:
        with open(TRACE) as f:
            for line in f:
                try:
                    out.append(json.loads(line))
                except ValueError:
                    pass
    except FileNotFoundError:
        pass
    return out


def wire(method, since=0):
    return [t for t in trace()[since:] if t.get("dir") == "out" and t.get("method") == method]


# ---------------------------------------------------------------- the server
class Serve:
    def __init__(self):
        self.proc = None

    def start(self, port):
        log = open(SERVE_LOG, "a")
        if SERVER == "live":
            live = os.environ["A12_LIVE_DIR"]
            env = dict(os.environ)
            with open(os.path.join(live, "token")) as f:
                env["OCTOS_AUTH_TOKEN"] = f.read().strip()
            cmd = [os.environ["A12_OCTOS_BIN"], "serve", "--port", str(port), "--host", "127.0.0.1", "--solo",
                   "--data-dir", os.path.join(live, "data"), "--instance-data-dir", os.path.join(live, "inst"),
                   "--cwd", os.path.join(live, "ws")]
            self.proc = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT, env=env, cwd=os.path.join(live, "ws"))
        else:
            cmd = [os.path.join(WS, "target/debug/examples/replay_serve"), str(port), "--scenario", "activity"]
            self.proc = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT)
        # Listening — and it is OUR process that listens (a port another lane
        # holds makes ours exit on bind; never drive someone else's server).
        import socket
        end = time.time() + 30
        while time.time() < end:
            if self.proc.poll() is not None:
                print(f"  server exited at start (port {port} taken?)", flush=True)
                return False
            try:
                socket.create_connection(("127.0.0.1", port), timeout=0.5).close()
                time.sleep(0.3)
                return self.proc.poll() is None
            except OSError:
                time.sleep(0.3)
        return False

    def kill(self, sig):
        if self.proc and self.proc.poll() is None:
            self.proc.send_signal(sig)
            try:
                self.proc.wait(timeout=20)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self.proc = None


# ---------------------------------------------------------------- the app
def launch_app(serve_port):
    env = dict(os.environ)
    for k, v in {
        "OCTOSCODE_DRAFTS_FILE": "drafts.json", "OCTOSCODE_CREDENTIALS_DIR": "cred",
        "OCTOSCODE_PREF_PATH": "prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": "notifications.json",
        "OCTOSCODE_RECENTS_DIR": "recents", "OCTOSCODE_SHOW_THINKING_FILE": "show-thinking.json",
        "OCTOSCODE_DOWNLOAD_DIR": "downloads", "OCTOSCODE_DISPLAY_PREFS_PATH": "display-v1.json",
        # Brief §8 (extended after A10): the pane's advanced file and the
        # per-install driver id too (outer/scripts/iso-env.sh).
        "OCTOSCODE_PANE_ADVANCED_FILE": "pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": "driver-id",
    }.items():
        env[k] = os.path.join(STATE, v)
    env.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{serve_port}",
        "OCTOS_PROFILE_ID": PROFILE,
        "OCTOSCODE_TRACE_FILE": TRACE,
        "OCTOSCODE_DESIGN_DIR": os.path.join(WS, "design"),
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": os.path.join(STATE, "hs"),
        "HEADLESS_ARGS": "--module octoscode",
    })
    if serve_port is None:
        # The transport-less shell (the judge tour's live tour seed).
        env.pop("OCTOS_BASE_URL", None)
        env["OCTOSCODE_SYNTHETIC_LIVE"] = "1"
    if SERVER == "live":
        live = os.environ["A12_LIVE_DIR"]
        with open(os.path.join(live, "token")) as f:
            env["OCTOS_BEARER"] = f.read().strip()
        env["OCTOS_WORKSPACE_CWD"] = os.path.join(live, "ws")
    if MODE == "phone":
        env["OCTOSENSE_WINDOW_SIZE"] = "360x780"
        env["HEADLESS_ARGS"] = "--module octoscode --test-action page:0 --test-action launch-octoscode"
    r = subprocess.run(["bash", os.path.join(WS, "harness/headless.sh"), "start", HOSTBIN, str(PORT)],
                       env=env, cwd=WS, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    print("\n".join(l for l in r.stdout.splitlines() if "listening" in l or "up:" in l), flush=True)
    return r.returncode == 0, env


def stop_app(env):
    subprocess.run(["bash", os.path.join(WS, "harness/headless.sh"), "stop", str(PORT)], env=env, cwd=WS,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


# ---------------------------------------------------------------- checks
def inside(inner, outer, slack=1.0):
    return (inner[0] >= outer[0] - slack and inner[1] >= outer[1] - slack
            and inner[0] + inner[2] <= outer[0] + outer[2] + slack
            and inner[1] + inner[3] <= outer[1] + outer[3] + slack)


def server_label(port):
    return f"127.0.0.1:{port}"


def outage_checks(tag, port):
    # The banner is laid out (its first frame) before it is measured.
    soon(lambda: rect("a12_link_detail") is not None and rect("a12_link_retry") is not None, 5)
    s = snap()
    banner = rect("a12_link_banner", s)
    check(f"{tag}: the banner shows", banner is not None, f"{banner}")
    check(f"{tag}: 'Reconnecting to Octos'", text("a12_link_title", s) == "Reconnecting to Octos", f"{text('a12_link_title', s)!r}")
    detail = text("a12_link_detail", s) or ""
    check(f"{tag}: the detail names the server in use + the retry count",
          detail.startswith("Connection lost") and server_label(port) in detail and "50190" not in detail, repr(detail))
    check(f"{tag}: the shell stays (no Connect card)",
          not shown("connect_card", s) and shown("conversation_column", s) and shown("i0_composer_0", s))
    if MODE == "phone":
        check(f"{tag}: the drawer trigger stays (phone)", shown("hd_menu", s) or shown("sidebar_toggle_hit", s))
    else:
        check(f"{tag}: the sidebar stays (desktop)", shown("sidebar_dock", s) and shown("sb_r_title", s))
    check(f"{tag}: the strip says Reconnecting", text("b3_strip_state", s) == "Reconnecting", f"{text('b3_strip_state', s)!r}")
    # Numeric layout: the banner IS the composer column's width, inside the
    # module; its texts inside it; its controls >= 28 px; above the strip.
    comp = rect("composer_row", s)
    root = rect("conversation_column", s)
    if banner and comp:
        check(f"{tag}: banner spans the composer column (x/w ±1)",
              abs(banner[0] - comp[0]) <= 1 and abs(banner[2] - comp[2]) <= 1, f"banner {banner} composer {comp}")
    if banner and root:
        check(f"{tag}: banner inside the conversation column", inside(banner, root), f"{banner} in {root}")
    for wid in ("a12_link_title", "a12_link_detail", "a12_link_mark"):
        r = rect(wid, s)
        check(f"{tag}: {wid} inside the banner", r is not None and banner is not None and inside(r, banner), f"{r}")
    for wid in ("a12_link_retry", "a12_link_leave"):
        r = rect(wid, s)
        check(f"{tag}: {wid} >= 28 px and inside the banner",
              r is not None and r[3] >= 28 and banner is not None and inside(r, banner), f"{r}")
    strip = rect("strip_row", s)
    if banner and strip:
        check(f"{tag}: banner sits above the strip (no overlap)", banner[1] + banner[3] <= strip[1] + 0.5, f"{banner} / {strip}")
    title, det = rect("a12_link_title", s), rect("a12_link_detail", s)
    if title and det:
        check(f"{tag}: title above detail, no overlap", title[1] + title[3] <= det[1] + 0.5, f"{title} / {det}")
    return s


def last_turn_id():
    starts = wire("turn/start")
    return (starts[-1].get("body") or {}).get("turn_id") if starts else None


def turn_terminal(turn_id):
    """The server's terminal for `turn_id` reached the app (the trace's
    inbound `projection/envelope` turn_terminal, or a `turn/completed`)."""
    for t in trace():
        if t.get("dir") != "in":
            continue
        b = t.get("body") or {}
        if b.get("turn_id") != turn_id:
            continue
        if t.get("method") == "turn/completed":
            return True
        if t.get("method") == "projection/envelope" and (b.get("payload") or {}).get("type") == "turn_terminal":
            return True
    return False


def wait_turn_done(turn_id, secs=180):
    """A live turn finished: its terminal arrived and the strip reads Ready
    again (the transcript list virtualizes, so rows are not counted)."""
    return bool(turn_id) and soon(lambda: turn_terminal(turn_id) and text("b3_strip_state") == "Ready", secs, 1.0)


def send_prompt(prompt):
    click("i0_composer_0")
    clear_composer()
    type_text(prompt)
    n0 = len(wire("turn/start"))
    click("send_hit")
    ok = soon(lambda: len(wire("turn/start")) > n0, 15)
    check(f"turn/start sent for {prompt[:40]!r}", ok)
    return ok


def run_synthetic():
    """A12_SERVER=none: a transport-less shell (OCTOSCODE_SYNTHETIC_LIVE, the
    judge tour's live tour). A typed command and a send with NO conversation
    used to do nothing at all; the banner says so and the text stays."""
    check("the synthetic shell is up", soon(lambda: shown("i0_composer_0") and not shown("connect_card"), 60))
    # The phone's first focus re-lays the composer out (a remount replaces
    # its input): focus, settle, focus again, then type and verify.
    click("i0_composer_0")
    time.sleep(1.0)
    click("i0_composer_0")
    clear_composer()
    type_text("/undo")
    if not soon(lambda: composer_text() == "/undo", 3):
        click("i0_composer_0")
        clear_composer()
        type_text("/undo")
    check("/undo typed", soon(lambda: composer_text() == "/undo", 3), repr(composer_text()))
    key("return")
    check("typed /undo + Return with no conversation: 'Not connected to Octos'",
          soon(lambda: text("a12_link_title") == "Not connected to Octos", 5), repr(text("a12_link_title")))
    check("…'/undo was not run — your text stays in the composer.'",
          text("a12_link_held") == "/undo was not run — your text stays in the composer.", repr(text("a12_link_held")))
    check("…the typed command stays", composer_text() == "/undo", repr(composer_text()))
    shot(f"{MODE}-0-not-connected")
    click("i0_composer_0")
    clear_composer()
    type_text("hello there")
    click("send_hit")
    check("a send with no conversation: 'Not sent — your text stays in the composer.'",
          soon(lambda: text("a12_link_held") == "Not sent — your text stays in the composer.", 5), repr(text("a12_link_held")))
    check("…the text stays", composer_text() == "hello there", repr(composer_text()))
    s = snap()
    banner, comp = rect("a12_link_banner", s), rect("composer_row", s)
    check("the notice banner spans the composer column", banner is not None and comp is not None
          and abs(banner[0] - comp[0]) <= 1 and abs(banner[2] - comp[2]) <= 1, f"{banner} / {comp}")
    d = rect("a12_link_dismiss", s)
    check("Dismiss >= 28 px", d is not None and d[3] >= 28, f"{d}")
    click("a12_link_dismiss")
    check("Dismiss clears the notice", soon(lambda: not shown("a12_link_banner"), 4))


def main():
    if not HOSTBIN:
        print("A12_HOSTBIN is required")
        sys.exit(2)
    if SERVER == "none":
        ok, env = launch_app(None)
        check("app launched (synthetic shell, no transport)", ok)
        try:
            run_synthetic()
        finally:
            stop_app(env)
            failed = [n for n, ok, _ in RESULTS if not ok]
            print(f"== WALK a12 no-conversation {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed", flush=True)
            sys.exit(1 if failed else 0)
    serve = Serve()
    check("server up", serve.start(SPORT), f"port {SPORT}")
    ok, env = launch_app(SPORT)
    check("app launched (hidden, isolated state)", ok)
    try:
        run(serve)
    finally:
        stop_app(env)
        serve.kill(signal.SIGTERM)
        failed = [n for n, ok, _ in RESULTS if not ok]
        print(f"== WALK a12 reconnect {MODE} ({SERVER}, {'KILL' if KILL == signal.SIGKILL else 'TERM'}"
              f"{', port change' if NEW_PORT else ''}): {len(RESULTS) - len(failed)}/{len(RESULTS)} passed", flush=True)
        print(f"   state dir (trace, serve log): {STATE}")
        sys.exit(1 if failed else 0)


def run(serve):
    check("the shell is up and live", soon(lambda: shown("i0_composer_0") and not shown("connect_card"), 60))
    check("strip Ready", soon(lambda: text("b3_strip_state") == "Ready", 30), f"{text('b3_strip_state')!r}")
    session0 = [t["body"].get("session_id") for t in wire("session/open")]
    session0 = session0[-1] if session0 else None
    title0 = text("hd_title")
    if TURNS:
        send_prompt("Reply with one short sentence: what is 2 + 3?")
        check("live turn 1 streamed and finished (its terminal arrived, strip Ready)", wait_turn_done(last_turn_id()))
        shot(f"{MODE}-live-turn")
    # An unsent draft.
    click("i0_composer_0")
    clear_composer()
    type_text(DRAFT)
    check("the unsent draft is in the composer", soon(lambda: composer_text() == DRAFT, 4), repr(composer_text()))
    shot(f"{MODE}-1-live")

    # ---- the server dies -------------------------------------------------
    log_since()
    t_kill = time.time()
    serve.kill(KILL)
    n_turns = len(wire("turn/start"))
    check("outage: the banner appears", soon(lambda: shown("a12_link_banner"), 15), f"after {time.time() - t_kill:.1f}s")
    outage_checks("outage", SPORT)
    check("outage: the draft is intact", composer_text() == DRAFT, repr(composer_text()))
    shot(f"{MODE}-2-reconnecting")
    # Send while down: refused on the banner, the text kept, nothing sent.
    click("send_hit")
    check("send while down: refused on the banner",
          soon(lambda: text("a12_link_held") == "Not sent — your text stays in the composer.", 5), repr(text("a12_link_held")))
    check("send while down: the draft is kept", composer_text() == DRAFT, repr(composer_text()))
    check("send while down: no turn/start reached the wire", len(wire("turn/start")) == n_turns)
    held = rect("a12_link_held")
    banner = rect("a12_link_banner")
    check("held line inside the banner", held is not None and banner is not None and inside(held, banner), f"{held} in {banner}")
    shot(f"{MODE}-3-send-refused")
    if RETRY == "during":
        # Retry now while still down: a fresh transport, the banner stays.
        log_since()
        check("Retry now is clickable", click("a12_link_retry"))
        lines = log_since()
        check("Retry now re-dials (a fresh transport)", any("retry now" in l and "re-dialing" in l for l in lines), "")
        check("…and the banner stays while the server is down", soon(lambda: shown("a12_link_banner"), 3))
    if DOWN_S > 7:
        a0 = text("a12_link_detail") or ""
        time.sleep(DOWN_S - 7)
        a1 = text("a12_link_detail") or ""
        check(f"long outage ({DOWN_S:.0f}s): still the banner, never the Connect card",
              shown("a12_link_banner") and not shown("connect_card"), f"{a0!r} -> {a1!r}")
        shot(f"{MODE}-3b-long-outage")
    else:
        time.sleep(max(0.0, DOWN_S - (time.time() - t_kill)))

    # ---- the server returns ------------------------------------------------
    if NEW_PORT:
        port_change(serve, session0)
        return
    n_open = len(wire("session/open"))
    n_trace = len(trace())
    check("server back on the same port", serve.start(SPORT))
    t_back = time.time()
    if RETRY == "after":
        # The server is back while the transport sleeps out its backoff (up to
        # 30 s): Retry now re-dials at once.
        time.sleep(1.5)
        if shown("a12_link_banner"):
            log_since()
            check("server back: Retry now is clickable", click("a12_link_retry"))
            check("…it re-dials at once and the banner goes", soon(lambda: not shown("a12_link_banner"), 10),
                  f"{time.time() - t_back:.1f}s after the restart")
        else:
            check("server back: already recovered before Retry now was needed", True, f"{time.time() - t_back:.1f}s")
    else:
        check("recovered: the banner goes by itself (the transport's own re-dial)",
              soon(lambda: not shown("a12_link_banner"), 45), f"{time.time() - t_back:.1f}s")
    check("recovered: strip Ready", soon(lambda: text("b3_strip_state") == "Ready", 20), repr(text("b3_strip_state")))
    reopen = [t for t in wire("session/open")[n_open:] if t["body"].get("reconnect")]
    check("recovered: the SAME Session re-opened ({reconnect: true})",
          bool(reopen) and reopen[-1]["body"].get("session_id") == session0, f"{[t['body'] for t in reopen]} vs {session0!r}")
    hyd = [t for t in trace()[n_trace:] if t.get("method") == "session/hydrate" and t.get("dir") == "in"]
    check("recovered: and re-hydrated (the canonical history came back)",
          any((t.get("body") or {}).get("session_id") == session0 for t in hyd), f"{len(hyd)} hydrate replies")
    check("recovered: the header names the same conversation", text("hd_title") == title0, f"{text('hd_title')!r} vs {title0!r}")
    if TURNS:
        texts = [w.get("t") or "" for w in snap() if w.get("v", 1) != 0 and w["r"][2] > 0]
        check("recovered: the timeline survived (turn 1 is still there)",
              any("what is 2 + 3" in t or "2 + 3 = 5" in t for t in texts))
        check("recovered: the re-hydrate did not duplicate turn 1",
              sum("what is 2 + 3" in t for t in texts) <= 1 and sum(t.strip().startswith("2 + 3 = 5") for t in texts) <= 1,
              f"{sum('what is 2 + 3' in t for t in texts)} prompts, {sum(t.strip().startswith('2 + 3 = 5') for t in texts)} answers")
    check("recovered: the unsent draft survived", composer_text() == DRAFT, repr(composer_text()))
    check("recovered: no Connect card at any point", not shown("connect_card"))
    shot(f"{MODE}-4-recovered")
    if TURNS:
        n0 = len(wire("turn/start"))
        click("send_hit")
        check("the draft goes out as the next prompt", soon(lambda: len(wire("turn/start")) > n0, 15))
        check("…and it streams to the end (its terminal arrived, strip Ready)", wait_turn_done(last_turn_id()))
        shot(f"{MODE}-5-next-turn")

    # ---- explicit give-up ----------------------------------------------------
    give_up(serve)


def give_up(serve):
    click("i0_composer_0")
    clear_composer()
    type_text(DRAFT)
    serve.kill(KILL)
    check("give-up: the outage again", soon(lambda: shown("a12_link_banner"), 15))
    if SERVER == "replay":
        # A typed command while down: refused on the banner, the text kept.
        click("i0_composer_0")
        clear_composer()
        type_text("/review")
        key("return")
        check("typed /review while down: refused on the banner",
              soon(lambda: text("a12_link_held") == "/review was not run — your text stays in the composer.", 5),
              repr(text("a12_link_held")))
        check("…the typed command stays in the composer", soon(lambda: composer_text() == "/review", 3), repr(composer_text()))
        shot(f"{MODE}-6-command-refused")
        # (No Escape: on the phone shell it is the back gesture.) Typing a
        # plain prompt over the palette dismisses it.
        click("i0_composer_0")
        clear_composer()
        type_text(DRAFT)
    log_since()
    check("banner Disconnect is clickable", click("a12_link_leave"))
    if soon(lambda: shown("a9_lv_dialog"), 4):
        check("the unsaved draft asks first (A9's confirmation)", text("a9_lv_title") == "Disconnect from Octos?", repr(text("a9_lv_title")))
        shot(f"{MODE}-7-disconnect-confirm")
        click("a9_lv_confirm")
    check("give-up: the Connect card returns", soon(lambda: shown("connect_card"), 10))
    srv = text("connect_server")
    check("…showing the server IN USE (not the default)", srv == f"http://127.0.0.1:{SPORT}", repr(srv))
    shot(f"{MODE}-8-connect-card")
    check("server back", serve.start(SPORT))
    click("connect_btn")
    check("the card's Connect dials that same server and the shell returns",
          soon(lambda: shown("i0_composer_0") and not shown("connect_card"), 30))
    shot(f"{MODE}-9-reconnected-from-card")


def port_change(serve, session0):
    """The server comes back on ANOTHER port: the app keeps naming the old
    server (never a default) until the user re-points it."""
    check("server back on a NEW port", serve.start(NEW_PORT), f"{NEW_PORT}")
    time.sleep(12)
    s = snap()
    detail = text("a12_link_detail", s) or ""
    check("moved: still 'Reconnecting', naming the OLD server (no silent fallback)",
          shown("a12_link_banner", s) and server_label(SPORT) in detail and "50190" not in detail and not shown("connect_card", s),
          repr(detail))
    shot(f"{MODE}-4-server-moved")
    click("a12_link_leave")
    if soon(lambda: shown("a9_lv_dialog"), 4):
        shot(f"{MODE}-5-disconnect-confirm")
        click("a9_lv_confirm")
    check("re-point: the Connect card shows the address it was using", soon(lambda: text("connect_server") == f"http://127.0.0.1:{SPORT}", 10),
          repr(text("connect_server")))
    shot(f"{MODE}-6-connect-card-old-address")
    click("connect_server")
    get("/k?c=end&wait=1")
    for _ in range(len(f"http://127.0.0.1:{SPORT}") + 2):
        get("/k?c=backspace&wait=1")
    type_text(f"http://127.0.0.1:{NEW_PORT}")
    check("re-point: the new address typed", soon(lambda: text("connect_server") == f"http://127.0.0.1:{NEW_PORT}", 4), repr(text("connect_server")))
    click("connect_btn")
    check("re-point: connected to the new port (shell back)", soon(lambda: shown("i0_composer_0") and not shown("connect_card"), 40))
    if TURNS:
        # Not a check: a FRESH connect opens the Session without loading its
        # history (pre-existing: only a reconnect / resync re-hydrates; the
        # web hydrates every candidate open). Recorded for the report.
        back = soon(lambda: any("2 + 3" in (w.get("t") or "") for w in snap() if w.get("v", 1) != 0), 8)
        print(f"NOTE re-point: turn 1 {'is' if back else 'is NOT'} shown after the fresh connect "
              "(first-open history load is outside A12)", flush=True)
    opens = [t["body"].get("session_id") for t in wire("session/open")]
    check("re-point: the Session opened on the new port is the one in use before", bool(opens) and opens[-1] == session0,
          f"{opens[-1:]} vs {session0!r}")
    shot(f"{MODE}-7-repointed")


if __name__ == "__main__":
    main()
