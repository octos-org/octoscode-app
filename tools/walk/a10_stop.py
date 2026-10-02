#!/usr/bin/env python3
"""A10 — the composer's Stop control by CLICK (web `TurnStopButton`: a
capability-gated Stop that interrupts the active turn and locks while the
interrupt is in flight; "Starting…" is status, not an interrupt).

Against `replay_serve --scenario a10 --slow turn/start=3000 --slow
turn/interrupt=2500`: Core accepts the start 3 s late (Starting… — the
control greyed and inert, a press sends nothing), the turn then stays live
(Stop), the interrupt is answered 2.5 s late (Stopping… — locked, a second
press sends nothing) and then r26's recorded interrupted terminal ends it
(the control is the send arrow again; the prompt comes back to the composer).
usage: OCTOSCODE_APP_BIN=<host octosense> a10_stop.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/stop/{MODE}"
PROMPT = "audit the parser"


def shown(W: Walk, wid: str) -> bool:
    return bool(W.visible(wid))


def press_control(W: Walk) -> None:
    r = W.rect("send_hit")
    if r:
        W.note(f"CLICK send_hit r={r}")
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)


def walk(W: Walk) -> None:
    W.note("== 1. a prompt: Starting… until Core accepts it")
    c = W.composer()
    if c is not None:
        x, y, w, h = c["r"]
        W.click_xy(x + w / 2, y + h / 2)
        W.key("End")
        W.clear_field(40)
        W.type_text(PROMPT)
    W.dismiss_keyboard("i0_composer_4")
    W.check("idle: the control is the send arrow", shown(W, "composer_send_icon") and not shown(W, "composer_stop_busy"))
    press_control(W)
    W.check("starting: the control is greyed (inert) and the activity row says 'Starting…'",
            W.wait(lambda: shown(W, "composer_stop_busy") and W.has_text("Starting…"), 3))
    W.shot(f"01-starting-{MODE}")
    W.mark()
    press_control(W)
    time.sleep(0.6)
    W.check("starting: a press sends no interrupt (status, not an interruptible turn)",
            W.replay_saw("turn/interrupt", 0) == 0)

    W.note("== 2. accepted: Stop")
    W.check("live: once Core accepts, the control is the black Stop (busy gone; 'Working')",
            W.wait(lambda: shown(W, "composer_stop_icon") and not shown(W, "composer_stop_busy")
                   and not W.has_text("Starting…"), 6))
    W.shot(f"02-stop-{MODE}")

    W.note("== 3. Stop: one interrupt, locked while it is in flight")
    press_control(W)
    W.check("stopping: the press sends ONE turn/interrupt; the control locks and the row says 'Stopping…'",
            W.wait(lambda: W.replay_saw("turn/interrupt", 0) == 1, 3)
            and W.wait(lambda: shown(W, "composer_stop_busy") and W.has_text("Stopping…"), 3))
    W.shot(f"03-stopping-{MODE}")
    press_control(W)
    time.sleep(0.6)
    W.check("stopping: a second press sends nothing more", W.replay_saw("turn/interrupt", 0) == 1,
            f"turn/interrupt x{W.replay_saw('turn/interrupt', 0)}")

    W.note("== 4. the interrupted terminal: idle again, the prompt back")
    W.check("ended: the control is the send arrow again",
            W.wait(lambda: shown(W, "composer_send_icon") and not shown(W, "composer_stop_icon")
                   and not shown(W, "composer_stop_busy"), 8))
    hits = W.visible("i0_composer_0")
    W.check("ended: the interrupted prompt is back in the composer (to resend or edit)",
            W.wait(lambda: any((h.get("val") or "") == PROMPT for h in W.visible("i0_composer_0")), 6),
            f"val={(hits[0].get('val') if hits else None)!r}")
    W.shot(f"04-ended-{MODE}")
    W.check("wire: exactly one turn/start and one turn/interrupt",
            W.replay_saw("turn/start", 0) == 1 and W.replay_saw("turn/interrupt", 0) == 1,
            f"turn/start x{W.replay_saw('turn/start', 0)}, turn/interrupt x{W.replay_saw('turn/interrupt', 0)}")


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10",
                         replay_args=["--slow", "turn/start=3000", "--slow", "turn/interrupt=2500"]))
