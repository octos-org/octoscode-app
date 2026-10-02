#!/usr/bin/env python3
"""A20 — parity row 250: an approval or question belongs to its EXACT origin
Session. The click walk, on the real app (hidden) against the a20 fixture
server (`a20_serve`: a prompt asking to run `rm -rf …` parks a typed approval,
"ask me" parks a question; a response on any other Session is refused).

  Session X raises an approval -> Session Y is opened by a sidebar CLICK:
  Y shows no card, its sidebar row does not wait (X's does), and Y's keyboard
  (Y / S / N pressed) sends nothing. Y raises its own question. Back on X (a
  CLICK): X's approval again (restored by X's canonical hydrate), and CLICK
  "Approve once" answers it with X's ids. Back on Y: Y's question survived X's
  approval and is answered with Y's ids.

Every control is CLICKED at its /snap rect; the wire is checked in the
fixture's request log. Captures (<out>/<mode>-NN-*.png) feed the UX gate.

  cargo build -p octoscode-module --example a20_serve
  OCTOSCODE_APP_BIN=<host-bin> python3 tools/walk/a20_interaction_walk.py desktop|phone [port] [fport] [out]
"""
import json
import sys
import time

from a10_lib import checks_line, dialog_checks
from a20_lib import run

WALK = {
    "name": "a20_interaction",
    "title": "interaction origin: an approval/question belongs to its Session (switch away, keyboard, back, answer)",
    "modes": ["desktop", "phone"],
    "rows": {250: ["Y: no card", "Y: the keyboard sends nothing", "X's row waits", "X: its approval again",
                   "wire: approval/respond to X", "Y: its question survived", "wire: user_question/respond to Y"]},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8483
FPORT = int(sys.argv[3]) if len(sys.argv) > 3 else 8485
OUT = sys.argv[4] if len(sys.argv) > 4 else "docs/ux/a20/walk"
X, XID = "Session X — clean the scratch dir", "a20:api:xray"
Y, YID = "Session Y — release notes", "a20:api:yankee"
COMMAND = "rm -rf scratch_dir"


def prompt(w, text):
    """CLICK the composer, type the prompt, Return (the production send)."""
    comp = w.composer()
    if comp is None:
        return False
    x, y, ww, h = comp["r"]
    w.click_xy(x + ww / 2, y + h / 2)
    w.type_text(text)
    w.key("Return")
    w.dismiss_keyboard()
    return True


def header_is(w, title, secs=8):
    return w.wait(lambda: w.has_text(title), secs)


def card_layout(w, name, frame_id, prefix):
    """The takeover card's numeric UX checks (a10_lib.dialog_checks): every
    label inside the card, no overlapping labels, controls >= 28 px, and the
    card inside the module view with >= 12 px gutters."""
    sn = w.snap()
    c = dialog_checks(sn, frame_id, prefix, module=w.module_rect(sn))
    mod, fr = w.module_rect(sn), c.get("frame")
    gutters = bool(mod and fr) and fr[0] - mod[0] >= 12 and (mod[0] + mod[2]) - (fr[0] + fr[2]) >= 12
    w.check(f"{name}: layout {checks_line(c)} gutters>=12={gutters}", bool(c.get("ok")) and gutters, json.dumps(c)[:240])


def waiting_dot_layout(w, title):
    """The Waiting dot sits ON its row: centred on the title's line (±3 px),
    left of the title, inside the sidebar."""
    sn = w.snap()
    row = next((s for s in sn if s.get("i") == "sb_r_title" and w.shown(s) and (s.get("t") or "").startswith(title[:12])), None)
    if row is None:
        return w.check(f"layout: the '{title[:12]}…' row is shown", False)
    cy = row["r"][1] + row["r"][3] / 2
    dots = [s["r"] for s in sn if s.get("i") == "sb_st_wait" and w.shown(s) and abs(s["r"][1] + s["r"][3] / 2 - cy) < 14]
    ok = bool(dots) and abs(dots[0][1] + dots[0][3] / 2 - cy) <= 3 and dots[0][0] + dots[0][2] <= row["r"][0]
    w.check("layout: X's Waiting dot is centred on X's row, left of its title", ok, f"dot={dots[:1]} title={row['r']}")


def walk(w):
    shot = lambda n: w.shot(f"{MODE}-{n}")
    w.sidebar_open()
    titles = lambda: [(s.get("t") or "") for s in w.snap() if s.get("i") == "sb_r_title" and w.shown(s)]
    w.check("the sidebar lists Session X and Session Y",
            w.wait(lambda: any(t.startswith("Session X") for t in titles()) and any(t.startswith("Session Y") for t in titles()), 15),
            str(titles()))

    # ---- Session X raises an approval ---------------------------------------
    w.check("CLICK row 'Session X' -> session/open X", w.open_row(X) and w.wait(lambda: len(w.wire("session/open", XID)) >= 1, 8))
    prompt(w, f"please run {COMMAND} in this repo")
    w.check("wire: X's turn/start", w.wait(lambda: len(w.wire("turn/start", XID)) == 1, 8))
    card = w.wait(lambda: bool(w.visible("cv_ap_card")) and COMMAND in (w.text("cv_ap_cmd") or ""), 12)
    w.check("X: the approval takes X's composer over", card, repr(w.text("cv_ap_cmd")))
    card_layout(w, "X's approval card", "cv_ap_card", ("cv_ap_",))
    shot("01-x-approval")

    # ---- Session Y on screen ------------------------------------------------
    w.check("CLICK row 'Session Y' -> session/open Y", w.open_row(Y) and w.wait(lambda: len(w.wire("session/open", YID)) >= 1, 8))
    w.check("Y: no card (X's approval is not Y's)",
            w.wait(lambda: not w.visible("cv_ap_card") and w.composer() is not None, 8))
    shot("02-y-no-card")
    # Y's keyboard: Y / S / N pressed in Y's composer.
    comp = w.composer()
    if comp:
        x, y, ww, h = comp["r"]
        w.click_xy(x + ww / 2, y + h / 2)
    w.mark()
    for k in ("y", "s", "n"):
        w.key(k)
    time.sleep(1.5)
    w.check("Y: the keyboard sends nothing (Y / S / N resolve to typing)",
            not w.wire("approval/respond"), str(w.wire("approval/respond")))
    w.check("log: Y's bare Y resolves to Ignore", w.logged("key KeyY -> Ignore", 3))
    w.dismiss_keyboard()
    # The sidebar: X waits, Y does not.
    w.sidebar_open()
    w.check("X's row waits (its own approval)", w.wait(lambda: w.row_status(X), 6))
    w.check("Y's row does not wait on X's approval", not w.row_status(Y))
    waiting_dot_layout(w, X)
    shot("03-sidebar-x-waits")
    if MODE == "phone":
        w.click("sidebar_toggle_hit")
        time.sleep(0.8)

    # ---- Y raises its own question -----------------------------------------
    prompt(w, "ask me which color to use")
    w.check("wire: Y's turn/start", w.wait(lambda: len(w.wire("turn/start", YID)) == 1, 8))
    w.check("Y: its own question takes Y's composer over",
            w.wait(lambda: bool(w.visible("cv_q_card")) and w.text("cv_q_title") == "Which color would you like to pick?", 12))
    card_layout(w, "Y's question card", "cv_q_card", ("cv_q_",))
    shot("04-y-question")

    # ---- back on X ----------------------------------------------------------
    w.check("CLICK row 'Session X' -> session/open X again", w.open_row(X) and w.wait(lambda: len(w.wire("session/open", XID)) >= 2, 8))
    w.check("wire: X's canonical hydrate read its parked interactions",
            w.wait(lambda: any(p.get("include") == ["pending_approvals"] for p in w.wire("session/hydrate", XID)), 8))
    again = w.wait(lambda: bool(w.visible("cv_ap_card")) and COMMAND in (w.text("cv_ap_cmd") or "") and not w.visible("cv_q_card"), 12)
    w.check("X: its approval again, never Y's question", again)
    card_layout(w, "X's restored approval card", "cv_ap_card", ("cv_ap_",))
    shot("05-x-approval-again")
    w.check("CLICK Approve once", w.click("cv_ap_once"))
    w.check("wire: exactly one approval/respond, to X with X's ids",
            w.wait(lambda: len(w.wire("approval/respond")) == 1, 8)
            and w.wire("approval/respond")[0].get("session_id") == XID
            and w.wire("approval/respond")[0].get("decision") == "approve"
            and w.wire("approval/respond")[0].get("approval_scope") == "request",
            str(w.wire("approval/respond")))
    w.check("X: decided, the composer is back", w.wait(lambda: not w.visible("cv_ap_card") and w.composer() is not None, 10))

    # ---- back on Y: its question survived -----------------------------------
    w.check("CLICK row 'Session Y' -> session/open Y again", w.open_row(Y) and w.wait(lambda: len(w.wire("session/open", YID)) >= 2, 8))
    w.check("Y: its question survived X's approval",
            w.wait(lambda: bool(w.visible("cv_q_card")) and w.text("cv_q_title") == "Which color would you like to pick?", 12))
    w.check("CLICK option 'Green'", w.click("cv_q_0_opt_1"))
    w.check("the primary is live", w.wait(lambda: bool(w.visible("cv_q_submit_box")), 5))
    w.check("CLICK Submit answer", w.click("cv_q_submit"))
    w.check("wire: user_question/respond to Y with Y's ids",
            w.wait(lambda: len(w.wire("user_question/respond")) == 1, 8)
            and w.wire("user_question/respond")[0].get("session_id") == YID
            and w.wire("user_question/respond")[0].get("answers") == [{"selected_labels": ["Green"]}],
            str(w.wire("user_question/respond")))
    w.check("Y: answered, the composer is back", w.wait(lambda: not w.visible("cv_q_card") and w.composer() is not None, 10))
    shot("06-y-answered")
    w.check("wire: nothing was ever answered on Y for X",
            all(p.get("session_id") == XID for p in w.wire("approval/respond")))


if __name__ == "__main__":
    sys.exit(run(walk, mode=MODE, outdir=OUT, port=PORT, fport=FPORT))
