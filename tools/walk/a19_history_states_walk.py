#!/usr/bin/env python3
"""A19b — what the conversation shows while a Session's history is not on
screen, walked in the real hidden app against RECORDED traffic (no model).

The walk owns both processes and always stops them: `replay_serve <rport>
--scenario history` (A15's r43a recording: the recorded Session's canonical
hydrate) with one A19b flag per run, and the hidden app (every app store in a
per-run dir, OCTOS_PROFILE_ID=dsflash: the dev/test override opens
`dsflash:main`).

  1. `--history-delay-ms 8000`: the history read answers 8 s late. From the
     first frame the instrument serves until the history arrives the
     conversation is "Loading conversation…" / "Restoring session state"
     (web `App.tsx:2564`, `:2741`), never the empty welcome; /snap geometry:
     the column centred in the conversation, clear of the header and the
     composer, no transcript row. Then the history replaces it.
  2. `--history-unknown`: every history read is refused "unknown session"
     (octos `-32100`). The client retries once in the folder the per-
     workspace catalog lists the Session under, is refused again, and says so:
     "Session recovery required" + the reason (web `App.tsx:2742-2750`) —
     never an empty transcript; the same geometry.

usage: python3 tools/walk/a19_history_states_walk.py <host-bin> <desktop|phone> [port] [replay-port] [out-dir]
Exit status 0 when every check passes. Captures: <out-dir>/<mode>-NN-<name>.png (+ .snap.json)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARY = sys.argv[1] if len(sys.argv) > 1 else ""
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
PORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8466
RPORT = int(sys.argv[4]) if len(sys.argv) > 4 else 8468
OUT = os.path.abspath(sys.argv[5]) if len(sys.argv) > 5 else os.path.join(ROOT, "docs/ux/a19/states")
PHONE = MODE == "phone"
BASE = f"http://127.0.0.1:{PORT}"
CWD = "/home/user/octos"
HOME_SESSION = "dsflash:main"
RESULTS = []
SHOT_N = [0]
STATE = [""]


def get(path, timeout=20):
    for attempt in range(4):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except urllib.error.HTTPError:
            if attempt == 3:
                raise
        except Exception:
            if attempt == 3:
                raise
        time.sleep(0.5)


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(s, wid):
    return [w for w in s if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def rect(wid, s=None):
    hits = sorted(visible(s if s is not None else snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[0]["r"] if hits else None


def text(wid, s=None):
    hits = sorted(visible(s if s is not None else snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[0].get("t", "") if hits else None


def wait(pred, secs=10.0, period=0.2):
    end = time.time() + secs
    while time.time() < end:
        try:
            if pred():
                return True
        except Exception:
            pass
        time.sleep(period)
    return False


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    print(f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else ""), flush=True)
    return ok


def shot(name, s=None):
    SHOT_N[0] += 1
    os.makedirs(OUT, exist_ok=True)
    p = os.path.join(OUT, f"{MODE}-{SHOT_N[0]:02d}-{name}.png")
    with open(p, "wb") as f:
        f.write(get("/g?raw=1", timeout=30))
    subprocess.run(["sips", "-Z", "1400", p, "--out", p], capture_output=True)
    # The geometry the checks read, beside the capture (no field values).
    keep = [{k: w.get(k) for k in ("i", "r", "t", "v")} for w in (s if s is not None else snap())
            if str(w.get("i", "")).startswith(("history_", "empty_", "hd_title", "i0_composer_0"))]
    with open(p[:-4] + ".snap.json", "w") as f:
        json.dump(keep, f, indent=1)
    print(f"  shot {os.path.relpath(p, ROOT)}", flush=True)


def trace():
    try:
        return [json.loads(l) for l in open(os.path.join(STATE[0], "trace.jsonl")) if l.strip()]
    except FileNotFoundError:
        return []


def frames(direction, method):
    return [t.get("body") or {} for t in trace() if t.get("dir") == direction and t.get("method") == method]


# ------------------------------------------------------------------ processes
def env():
    st = STATE[0]
    e = dict(os.environ)
    for k, v in {
        "OCTOSCODE_DRAFTS_FILE": "drafts.json", "OCTOSCODE_CREDENTIALS_DIR": "cred",
        "OCTOSCODE_PREF_PATH": "prefs.json", "OCTOSCODE_NOTIFICATIONS_FILE": "notifications.json",
        "OCTOSCODE_RECENTS_DIR": "recents", "OCTOSCODE_SHOW_THINKING_FILE": "show-thinking.json",
        "OCTOSCODE_DOWNLOAD_DIR": "downloads", "OCTOSCODE_DISPLAY_PREFS_PATH": "display-v1.json",
        "OCTOSCODE_PANE_ADVANCED_FILE": "pane-advanced.json", "OCTOSCODE_DRIVER_ID_PATH": "driver-id",
        "OCTOSCODE_CONNECTION_FILE": "connection-v1.json",
    }.items():
        e[k] = os.path.join(st, v)
    for d in ("cred", "downloads", "recents"):
        os.makedirs(os.path.join(st, d), exist_ok=True)
    e.update({
        "OCTOS_BASE_URL": f"http://127.0.0.1:{RPORT}",
        "OCTOS_BEARER": "walk-dummy-token",
        "OCTOS_PROFILE_ID": "dsflash",
        "OCTOSCODE_TRACE_FILE": os.path.join(st, "trace.jsonl"),
        "OCTOSCODE_DESIGN_DIR": os.path.join(ROOT, "design"),
        "MAKEPAD_WM_TEST_APP": "octoscode",
        "HEADLESS_STATE": os.path.join(st, "hs"),
        "HEADLESS_ARGS": "--module octoscode" + (" --test-action page:0 --test-action launch-octoscode" if PHONE else ""),
    })
    e.pop("OCTOS_WORKSPACE_CWD", None)
    if PHONE:
        e["OCTOSENSE_WINDOW_SIZE"] = "360x780"
    return e


def start_app():
    subprocess.run(["bash", os.path.join(ROOT, "harness/headless.sh"), "start", BINARY, str(PORT)],
                   env=env(), cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return wait(lambda: b'"sz"' in get("/s"), 90, 0.25)


def stop_app():
    subprocess.run(["bash", os.path.join(ROOT, "harness/headless.sh"), "stop", str(PORT)], env=env(), cwd=ROOT,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def start_replay(flag):
    replay = os.path.join(ROOT, "target/debug/examples/replay_serve")
    log = open(os.path.join(STATE[0], "replay.log"), "w")
    serve = subprocess.Popen([replay, str(RPORT), "--scenario", "history", *flag], stdout=log, stderr=subprocess.STDOUT)
    time.sleep(1.5)
    return serve


def stop_replay(serve):
    serve.terminate()
    try:
        serve.wait(timeout=10)
    except subprocess.TimeoutExpired:
        serve.kill()


# ------------------------------------------------------------------ checks
def rows_on_screen(s):
    """Any transcript row in view (the list opens at the bottom: the last
    turn's answer and tool cards, its prompt possibly above the fold)."""
    return any(any(k in str(w.get("i", "")) for k in ("_userbubble_", "_assistantprose", "_toolcell_"))
               and w.get("v", 1) != 0 and w["r"][3] > 0 for w in s)


def geometry(tag, s):
    """The state's column, by /snap numbers: centred in the conversation
    (the welcome's own splash), clear of the header and the composer."""
    col, title, hint = rect("history_col", s), rect("history_title", s), rect("history_hint", s)
    area = rect("empty_splash", s) or rect("empty_state", s)
    comp = rect("i0_composer_0", s)
    head = rect("hd_title", s)
    check(f"{tag}: the column, title and hint are laid out", bool(col and title and hint and area),
          f"col {col} title {title} hint {hint} area {area}")
    if not (col and title and hint and area):
        return
    cx = lambda r: r[0] + r[2] / 2  # noqa: E731
    check(f"{tag}: title centred in the conversation (|dx| <= 2)", abs(cx(title) - cx(area)) <= 2.0,
          f"title cx {cx(title):.1f}, area cx {cx(area):.1f}")
    check(f"{tag}: hint centred under it (|dx| <= 2)", abs(cx(hint) - cx(title)) <= 2.0,
          f"hint cx {cx(hint):.1f}, title cx {cx(title):.1f}")
    check(f"{tag}: the hint follows the title (gap 0-16 px)", 0 <= hint[1] - (title[1] + title[3]) <= 16,
          f"gap {hint[1] - (title[1] + title[3]):.1f}")
    check(f"{tag}: the column inside the conversation's width", col[0] >= area[0] - 1 and col[0] + col[2] <= area[0] + area[2] + 1,
          f"col {col} area {area}")
    if comp:
        check(f"{tag}: clear of the composer (hint bottom < composer top)", hint[1] + hint[3] < comp[1],
              f"hint bottom {hint[1] + hint[3]:.1f}, composer top {comp[1]:.1f}")
    if head:
        check(f"{tag}: clear of the header", title[1] > head[1] + head[3], f"title top {title[1]}, header bottom {head[1] + head[3]}")
    check(f"{tag}: no 'New chat defaults' strip over it (this is not a New chat)", not visible(s, "hd_defaults_text"),
          repr(text("hd_defaults_text", s)))


def run_loading():
    serve = start_replay(["--history-delay-ms", "8000"])
    try:
        check("loading: replay server up", serve.poll() is None)
        check("loading: app up", start_app())
        # From the first frame the instrument serves until the history is in.
        welcome = loading = n = 0
        first_loading = None
        end = time.time() + 30
        while time.time() < end:
            try:
                s = snap()
            except Exception:
                time.sleep(0.1)
                continue
            n += 1
            welcome += bool(visible(s, "empty_title"))
            if text("history_title", s) == "Loading conversation…":
                loading += 1
                if first_loading is None:
                    first_loading = s
                    time.sleep(0.6)
                    s = snap()
                    first_loading = s
                    shot("loading", s)
            if rows_on_screen(s):
                break
            time.sleep(0.15)
        check("loading: never the empty welcome, from the first frame until the history arrived",
              welcome == 0 and n > 0, f"{n} frames: {welcome} welcome, {loading} loading")
        check("loading: 'Loading conversation…' while the read is in flight", loading > 0)
        if first_loading is not None:
            check("loading: the hint is 'Restoring session state'",
                  text("history_hint", first_loading) == "Restoring session state", repr(text("history_hint", first_loading)))
            check("loading: no transcript row under it", not rows_on_screen(first_loading))
            geometry("loading", first_loading)
        s = snap()
        check("loading: the history replaced it (rows, no loading state)",
              rows_on_screen(s) and not visible(s, "history_title"))
        reads = [b for b in frames("out", "session/hydrate") if b.get("session_id") == HOME_SESSION and b.get("include") == ["messages"]]
        check("wire: ONE history read (it was only late)", len(reads) == 1, f"{len(reads)}")
        time.sleep(1.0)
        shot("history-arrived")
    finally:
        stop_app()
        stop_replay(serve)


def run_failed():
    serve = start_replay(["--history-unknown"])
    try:
        check("failed: replay server up", serve.poll() is None)
        check("failed: app up", start_app())
        welcome = n = 0
        end = time.time() + 40
        while time.time() < end:
            s = snap()
            n += 1
            welcome += bool(visible(s, "empty_title"))
            if text("history_title", s) == "Session recovery required":
                break
            time.sleep(0.15)
        check("failed: 'Session recovery required' once the retry is refused too",
              text("history_title", s) == "Session recovery required", repr(text("history_title", s)))
        check("failed: never the empty welcome on the way", welcome == 0, f"{n} frames: {welcome} welcome")
        hint = text("history_hint", s) or ""
        check("failed: the reason is shown (the server's message)", "unknown session: dsflash:main" in hint, repr(hint))
        check("failed: no transcript row", not rows_on_screen(s))
        refused = frames("in", "error:session/hydrate")
        opens = [b for b in frames("out", "session/open") if b.get("session_id") == HOME_SESSION]
        check("wire: refused, retried once in the catalog's folder, refused again",
              len([e for e in refused if e.get("code") == -32100]) >= 2 and len(opens) == 2 and bool(opens[1].get("cwd")),
              f"{len(refused)} refusals, opens {[bool(o.get('cwd')) for o in opens]}")
        time.sleep(0.6)
        s = snap()
        geometry("failed", s)
        shot("failed", s)
    finally:
        stop_app()
        stop_replay(serve)


def main():
    code = 1
    dirs = []
    try:
        for run in (run_loading, run_failed):
            STATE[0] = tempfile.mkdtemp(prefix="a19states.")
            dirs.append(STATE[0])
            run()
        failed = [r for r in RESULTS if not r[1]]
        print(f"== WALK a19 history states {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed", flush=True)
        code = 0 if not failed else 1
    finally:
        stop_app()
        for d in dirs:
            shutil.rmtree(d, ignore_errors=True)
    return code


if __name__ == "__main__":
    sys.exit(main())
