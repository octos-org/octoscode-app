#!/usr/bin/env python3
"""A18 — one Stop draws ONE "Turn stopped" notice, clicked in the real app.

    python3 tools/walk/a18_stop_walk.py <host-bin> <desktop|phone> <port> <rport> <outdir>

The A15 live smoke (docs/ux/a15-live/smoke/06-stopped.png) showed a stopped turn with its notice and a SECOND
identical notice under it. Its own trace explains the second: the startup hydrate returned no durable rows (the
serve's session store had been reset) while the replay window still held an earlier run's four completed turns and
its stop ("turn interrupted by client"); the history fold noted that stop (01-connected.png shows it before any
turn).

Against `replay_serve --scenario a10 --stale-window`: every `session/hydrate` answers with that first-launch hydrate
(fixture a18-stale-window-a6ea8505, cut from the smoke's trace), and the a10 seat simulator holds a turn live until
`turn/interrupt`, then sends r26's recorded interrupted terminal for the app's own turn. Each step is a CLICK:

1. the fresh chat over the reset store: NO "Turn stopped" (the stale stop is not noted);
2. type a prompt + Return: the turn is live (the Stop control shows);
3. CLICK Stop: `turn/interrupt` on the wire, the composer is idle again, exactly ONE "Turn stopped" notice, below the
   stopped prompt;
4. a second Stop-free look after a pause: still one.
"""
from __future__ import annotations

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from a10_lib import Walk, run_session  # noqa: E402

PROMPT = "Write a 400-word story about a lighthouse keeper and a storm."


def notices(W: Walk) -> list[dict]:
    return [w for w in W.snap() if Walk.shown(w) and str(w.get("i", "")).startswith("b3_tl_notice_title_")
            and (w.get("t") or "").strip() == "Turn stopped"]


def idle(W: Walk) -> bool:
    sn = W.snap()
    return bool(W.visible("composer_send_icon", sn)) and not W.visible("composer_stop_icon", sn) \
        and not W.visible("composer_stop_busy", sn)


def walk(W: Walk) -> None:
    W.note("== 1. the fresh chat over a reset store")
    W.wait(lambda: bool(W.visible("hd_tab_chat_hit")), 20)
    time.sleep(2.5)  # the open's hydrate (the stale window) folds
    W.check("the open hydrated the reset store's window", W.replay_saw("session/hydrate") >= 1)
    stale = notices(W)
    W.check("no 'Turn stopped' before any Stop (the earlier run's stop is not noted)", not stale,
            f"{len(stale)} notice(s): {[n['r'] for n in stale]}")
    W.shot(f"01-fresh-{W.mode}")

    W.note("== 2. a prompt, live")
    c = W.composer()
    if not W.check("the composer is shown", c is not None):
        return
    W.click_xy(c["r"][0] + 12, c["r"][1] + c["r"][3] / 2)
    W.type_text(PROMPT)
    W.key("Return")
    W.dismiss_keyboard()
    live = W.wait(lambda: bool(W.visible("composer_stop_icon")) or bool(W.visible("composer_stop_busy")), 15)
    W.check("Return -> turn/start; the turn is live (the Stop control shows)",
            live and W.replay_saw("turn/start") >= 1)

    W.note("== 3. one Stop")
    W.click("send_hit")
    W.check("CLICK Stop -> turn/interrupt on the wire", W.replay_saw("turn/interrupt") >= 1)
    W.check("the composer is idle again", W.wait(lambda: idle(W), 15))
    W.wait(lambda: bool(notices(W)), 10)
    time.sleep(1.0)
    shown = notices(W)
    W.check("one Stop, ONE 'Turn stopped' notice", len(shown) == 1, f"{len(shown)}: {[n['r'] for n in shown]}")
    bubble = [w for w in W.snap() if Walk.shown(w) and "userbubble" in str(w.get("i", ""))
              and PROMPT[:20] in (w.get("t") or "")]
    if shown and bubble:
        W.check("the notice sits below the stopped prompt", shown[0]["r"][1] > bubble[-1]["r"][1],
                f"notice y={shown[0]['r'][1]} prompt y={bubble[-1]['r'][1]}")
    W.shot(f"02-stopped-{W.mode}")

    W.note("== 4. settled")
    time.sleep(3.0)
    W.check("still one notice once the stream settles", len(notices(W)) == 1, f"{len(notices(W))}")


if __name__ == "__main__":
    if len(sys.argv) < 6:
        print(__doc__)
        sys.exit(2)
    BIN, MODE, PORT, RPORT, OUT = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, port=PORT, replay_port=RPORT, scenario="a10",
                         app_bin=BIN, replay_args=["--stale-window"]))
