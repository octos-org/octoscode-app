#!/usr/bin/env python3
"""A1 — the conversation click walk: every control the conversation pane
owns is reached by a CLICK at its laid-out rect (the LESSONS rule), and each
step asserts the app's own effect (a /snap widget or text, and the routed
log line). It finishes with the numeric layout checks (tools/a1/ux_checks.py).

The app must already run hidden with the instrument on PORT and the A1
capture seed (no transport, no model turn), e.g.

  OCTOSCODE_SYNTHETIC_LIVE=1 OCTOSCODE_SYNTHETIC_EMPTY=1 \\
  OCTOSCODE_SYNTHETIC_TOOLS=gfm OCTOSCODE_DESIGN_DIR=$PWD/design \\
  MAKEPAD_WM_TEST_APP=octoscode HEADLESS_ARGS="--module octoscode" \\
    bash harness/headless.sh start <host-bin> 8411
  python3 tools/a1/conversation_walk.py 8411 [--copy]
  bash harness/headless.sh stop 8411

The seed is one settled turn with three tool calls, then a settled turn
whose answer is the GFM sample (components::GFM_SAMPLE: heading, strong, a
table, an ordered list, raw HTML).
Exit status: 0 when every step passes.
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
# Desktop only: the seeded sidebar row is a drawer on the phone.
WALK = {
    "name": "a1_conversation",
    "title": "conversation pane: tool rows, Worked-for fold, session row, GFM answer, layout",
    "modes": ["desktop"],
    "app": {"env": {"OCTOSCODE_SYNTHETIC_LIVE": "1", "OCTOSCODE_SYNTHETIC_EMPTY": "1",
                    "OCTOSCODE_SYNTHETIC_TOOLS": "gfm"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}"]}],
    "timeout": 300,
    "rows": {
        215: {"checks": ["a tool header opens its output", "…and closes it",
                         "Worked for folds the turn's calls", "…and shows them again"],
              "partial": "reader-controlled disclosures on a seeded settled transcript (A6 walks the replayed "
                         "turns' folds); ten live streamed turns are not walked"},
    },
}

PORT = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 8411
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []
LOG_SEQ = [0]


def get(path, timeout=20):
    # A11: an input route with wait=1 answers HTTP 404 when its frame was
    # coalesced — the input was delivered; only a read may raise.
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read().decode()
    except urllib.error.HTTPError:
        if path.startswith(("/click", "/t?", "/k?", "/m?")):
            return ""
        raise


def snap():
    s = json.loads(get("/snap?all=1"))["s"]
    for w in s:
        w.pop("val", None)  # a TextInput's raw value can be a secret
    return s


def visible(widgets, pattern):
    rx = re.compile(pattern)
    return sorted(
        (w for w in widgets if rx.fullmatch(str(w.get("i") or "")) and w.get("v", 1) != 0
         and (w.get("r") or [0, 0, 0, 0])[2] > 0 and w["r"][3] > 0),
        key=lambda w: (w["r"][1], w["r"][0]),
    )


def shown(pattern):
    return bool(visible(snap(), pattern))


def texts(pattern):
    return [w.get("t", "") for w in visible(snap(), pattern)]


def click(pattern, nth=0):
    hits = visible(snap(), pattern)
    if len(hits) <= nth:
        return False
    x, y, w, h = hits[nth]["r"]
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.4)
    return True


def scroll(dy):
    lst = visible(snap(), "timeline_list")
    if lst:
        x, y, w, h = lst[0]["r"]
        get(f"/m?k=scroll&x={x + w / 2}&y={y + h / 2}&dy={dy}&wait=1")
        time.sleep(0.6)


def log_since():
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return d.get("l", [])


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))


def step(name, pattern, expect, nth=0, log_needle=None):
    log_since()
    clicked = click(pattern, nth)
    lines = log_since()
    ok = clicked and expect()
    if log_needle:
        ok = ok and any(log_needle in l for l in lines)
    check(name, ok, f"clicked {pattern}#{nth}={clicked}" + (f" log~{log_needle!r}" if log_needle else ""))


def layout(label):
    path = os.path.join(tempfile.gettempdir(), f"a1-walk-{PORT}.json")
    with open(path, "w") as f:
        json.dump({"s": snap()}, f)
    here = os.path.dirname(os.path.abspath(__file__))
    out = subprocess.run([sys.executable, os.path.join(here, "ux_checks.py"), path, "conversation"],
                         capture_output=True, text=True)
    last = (out.stdout.strip().splitlines() or ["(no output)"])[-1]
    check(f"layout ({label})", out.returncode == 0, last)
    os.remove(path)


def main():
    log_since()
    scroll(-4000)
    check("the transcript shows the seeded turn's tool card", len(visible(snap(), r"i\d+_toolcell")) == 3)
    step("a tool header opens its output", "tool_hit",
         lambda: shown(r"i\d+_toolcell_output"), log_needle="tool.toggle")
    step("…and closes it", "tool_hit",
         lambda: not shown(r"i\d+_toolcell_output"), log_needle="tool.toggle")
    step("Worked for folds the turn's calls", "worked_hit",
         lambda: len(visible(snap(), r"i\d+_toolcell")) == 0, log_needle="answer.expand")
    step("…and shows them again", "worked_hit",
         lambda: len(visible(snap(), r"i\d+_toolcell")) == 3, log_needle="answer.expand")
    layout("top, tool card")
    step("the session row reopens the conversation", "sb_r_open",
         lambda: shown(r"i\d+_assistantprose"), log_needle="session.open")
    scroll(4000)
    gfm = [t for t in texts(r"i\d+_assistantprose") if "# Workspace summary" in t]
    check("the GFM answer renders in ONE native Markdown region",
          len(gfm) == 1 and "| Path | Kind |" in gfm[0] and "**two**" in gfm[0],
          f"{len(gfm)} region(s) carry the sample")
    # The copy step writes the REAL system clipboard of the machine the
    # hidden app runs on, so it only runs when asked (`--copy`).
    if "--copy" in sys.argv:
        step("the answer's copy control copies its answer", "answer_copy_hit", lambda: True,
             nth=len(visible(snap(), "answer_copy_hit")) - 1, log_needle="answer.copy:")
    layout("bottom, GFM answer")
    failed = [n for n, ok, _ in RESULTS if not ok]
    print(f"== WALK conversation: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
