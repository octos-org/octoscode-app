#!/usr/bin/env python3
"""#30 — a pasted trailing newline (or Shift+Enter) never sends; only Enter does.

    OCTOSCODE_APP_BIN=<octoscode> A10_PORT=<port> A10_REPLAY_PORT=<rport> a37_paste_no_send.py desktop <outdir>

Against `replay_serve --scenario a10`. #36g item 2 made a trailing "\\n" in the composer's text change the SEND gesture
(the Android IME commits Enter as text), on every platform: a paste ending in a newline (a terminal line, a log line)
sent at once, and so did Shift+Enter at the end of a draft. The gate is now Android + exactly one "\\n" appended to the
previous draft (`screens::keys::ime_enter_submits`); on desktop Enter is the `ReturnKey` event.

`/t?t=...%0A` inserts text through the same `changed()` path a paste takes (the a97acc5b proof). Each step is a key or
text event, and the assertion is the wire: `turn/start` at the replay server.

1. type text, then insert text ending in a newline: the draft keeps the newline, NO turn/start, no IME-Enter submit;
2. Shift+Enter at the end of the draft: a newline is inserted, NO turn/start;
3. plain Enter: exactly ONE turn/start.
"""
from __future__ import annotations

import sys
import time

from a10_lib import Walk, run_session

WALK = {
    "name": "a37_paste_no_send",
    "title": "a pasted trailing newline and Shift+Enter stay in the composer; only Enter sends (#30)",
    "modes": ["desktop"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"],
              "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}", "OCTOSCODE_APP_BIN": "{bin}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 300,
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"tmp/walk/a37/{MODE}"
TYPED = "first line"
PASTED = "pasted from a terminal\n"


LOG: list[str] = []


def saw(W: Walk, needle: str, secs: float = 0.0) -> bool:
    """`needle` in any app log line since the walk began (waits up to `secs` for it)."""
    def has() -> bool:
        LOG.extend(W.log_since())
        return any(needle in l for l in LOG)

    return W.wait(has, secs) if secs else has()


def starts(W: Walk, secs: float = 0.0) -> int:
    return W.replay_saw("turn/start", secs) if secs else sum(
        1 for l in (W.replay_log.read_text().splitlines() if W.replay_log and W.replay_log.exists() else [])
        if "<- turn/start " in l)


def draft(W: Walk) -> str:
    c = W.composer()
    return (c.get("t") or "") if c else ""


def walk(W: Walk) -> None:
    W.note("== 1. a paste that ends with a newline")
    W.wait(lambda: bool(W.visible("hd_tab_chat_hit")), 20)
    time.sleep(2.5)
    c = W.composer()
    if not W.check("the composer is shown", c is not None):
        return
    W.click_xy(c["r"][0] + 12, c["r"][1] + c["r"][3] / 2)
    W.type_text(TYPED)
    W.type_text(PASTED)
    time.sleep(1.0)
    W.check("the paste reached the draft (draft synced)", saw(W, "draft synced: 33 chars", 3))
    W.check("no 'composer newline -> submit'", not saw(W, "composer newline -> submit"))
    W.check("NO turn/start from the paste", starts(W, 1.5) == 0, f"{starts(W)}")
    W.check("the draft keeps the pasted text and its trailing newline",
            draft(W) == TYPED + PASTED, repr(draft(W)))
    W.shot(f"01-pasted-{W.mode}")

    W.note("== 2. Shift+Enter at the end of the draft")
    W.get("/k?c=ReturnKey&shift=1&wait=1", tolerant=True)
    time.sleep(1.0)
    W.check("Shift+Enter inserted a newline at the end", draft(W) == TYPED + PASTED + "\n", repr(draft(W)))
    W.check("no 'composer newline -> submit' after Shift+Enter", not saw(W, "composer newline -> submit"))
    W.check("NO turn/start from Shift+Enter", starts(W, 1.5) == 0, f"{starts(W)}")

    W.note("== 3. plain Enter")
    W.key("ReturnKey")
    W.check("Enter -> turn/start", starts(W, 10) >= 1)
    time.sleep(1.5)
    W.check("exactly ONE turn/start", starts(W) == 1, f"{starts(W)}")
    W.shot(f"02-sent-{W.mode}")


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
