#!/usr/bin/env python3
"""A10 — the Fleet click walk (A4 board-3 Fleet surface; web `FleetView.tsx`,
`fleet-actions.ts`, `SessionControlBar.tsx`, `PeerControlPanel.tsx`).

Every control is reached by a CLICK at its laid-out rect: the sidebar
footer's Fleet entry opens the pane (phone: the drawer first); the lane
picker, the brief, Start, the row actions (Approve / Steer / Stop), the
per-group Finished (n) disclosure, Peer gather, Advanced (the driver
disclosure, Release / Acquire seat) and the control seat run through the
real app against `replay_serve --scenario fleet` — the FAITHFUL
external-driver fixture (`a10-fleet-driver-synthetic.jsonl`, synthetic: no
live server advertises `external_driver_v1`), answered per request by the
replay's simulator. Each step asserts the app's own effect (a /snap widget
or text and the routed log line); the replay log proves each frame and the
ids it carried.

usage: OCTOSCODE_APP_BIN=<host octosense> a10_fleet.py <desktop|phone> <outdir>
"""
import os
import re
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, inside, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/fleet/{MODE}"
# A10's app port is 8420; the replay port is free per run (another agent's
# app once held 8431).
PORT = int(os.environ.get("A10_FLEET_PORT", "8420"))
REPLAY = int(os.environ.get("A10_FLEET_REPLAY", "8434"))


def where(W: Walk, wid: str) -> str:
    """`in` (wholly inside the viewport), `cut-top` / `cut-bottom` (a scroll
    edge cuts it: /snap reports only the visible part of a clipped widget, so
    a rect touching an edge is cut), or `out` (not drawn)."""
    sn = W.snap()
    vp, r = W.rect("b3_scroll", sn=sn), W.rect(wid, sn=sn)
    if not (vp and r and inside(r, vp)):
        return "out"
    if r[1] <= vp[1] + 1:
        return "cut-top"
    if r[1] + r[3] >= vp[1] + vp[3] - 1:
        return "cut-bottom"
    return "in"


def wholly(W: Walk, wid: str) -> bool:
    return where(W, wid) == "in"


def wheel(W: Walk, dy: float) -> None:
    vp = W.rect("b3_scroll")
    if vp:
        # In the pane's empty left gutter: on a phone the wheel is a touch drag,
        # and a drag that starts on a field focuses it (the soft keyboard).
        W.get(f"/m?k=scroll&x={vp[0] + 6:.0f}&y={vp[1] + vp[3] / 3:.0f}&dy={dy:.0f}&wait=1", tolerant=True)
        time.sleep(0.2)


def seek(W: Walk, wid: str, steps: int = 40) -> bool:
    """Bring `wid` wholly into the pane's scroll viewport with the user's
    wheel: from the top, then down in small steps (content scrolled out of a
    ScrollYView is not drawn, so an unseen target is searched for; a remount
    may have reset the offset, so the search always starts at the top)."""
    if wholly(W, wid):
        return True
    for _ in range(6):
        wheel(W, -600)
    last = None
    for _ in range(steps):
        at = where(W, wid)
        if at == "in":
            return True
        r = W.rect(wid)
        if at.startswith("cut") and r == last:
            return True  # the scroll is at its end: this IS the whole widget
        last = r
        wheel(W, -60 if at == "cut-top" else 60 if at == "cut-bottom" else 90)
    ok = wholly(W, wid)
    if not ok:
        W.note(f"SEEK {wid} — not found")
    return ok


def shown(W: Walk, wid: str) -> bool:
    return seek(W, wid)


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0) -> bool:
    time.sleep(0.4)  # let a remount from the previous reply settle
    W.mark()
    ok = seek(W, wid) and W.click(wid)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = seek(W, wid) and W.click(wid)
        logged = W.logged(needle, secs / 2) if ok else False
    seen = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and seen


# The phone shell's soft keyboard: its own hide key (the chevron at the
# keyboard bar's right end, `PhoneHit::HideKeyboard`, mobile_surface.rs) in
# the 402x874 phone window. While the keyboard is up the shell lifts the
# module's content above it, so /snap rects are off until it is hidden; with
# the keyboard down this point is outside the module view (x > 360).
PHONE_HIDE_KEYBOARD = (377, 573)


def hide_keyboard(W: Walk) -> None:
    if W.mode == "phone":
        W.note("TAP the keyboard's hide key")
        W.click_xy(*PHONE_HIDE_KEYBOARD)
        time.sleep(0.6)


def type_into(W: Walk, wid: str, text: str, neutral: str = "b3_title") -> None:
    """Focus `wid`, type, then (phone) put the soft keyboard away with its own
    hide key, as a person does before reaching a control under it. `neutral`
    is kept for the call sites' readability."""
    seek(W, wid)
    r = W.rect(wid)
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        W.type_text(text)
        hide_keyboard(W)


def replay_lines(W: Walk, needle: str) -> list[str]:
    if not W.replay_log or not W.replay_log.exists():
        return []
    return [l for l in W.replay_log.read_text().splitlines() if needle in l]


def row_of(W: Walk, label_prefix: str) -> int | None:
    """The drawn index of the row whose label starts with `label_prefix`
    (scrolled to: rows below the fold are not drawn)."""
    for w in W.prefixed("b3_fleet_row_"):
        m = re.fullmatch(r"b3_fleet_row_(\d+)_label", str(w.get("i")))
        if m and (w.get("t") or "").startswith(label_prefix):
            return int(m.group(1))
    for i in range(6):
        if seek(W, f"b3_fleet_row_{i}_label", steps=30) and W.text(f"b3_fleet_row_{i}_label").startswith(label_prefix):
            return i
    return None


def status_of(W: Walk, i: int) -> str:
    seek(W, f"b3_fleet_row_{i}_status", steps=30)
    return W.text(f"b3_fleet_row_{i}_status")


def numeric(W: Walk, name: str):
    sn = W.snap()
    c = dialog_checks(sn, "b3_fleet_panel", ("b3_fleet_", "b3_title"), viewport="b3_scroll")
    # A control the scroll viewport's edge cuts reports its CLIPPED rect (a
    # few px tall) — clipped by the scroll by design, not an undersized hit.
    vp = next((w["r"] for w in sn if w.get("i") == "b3_scroll" and Walk.shown(w)), None)
    if vp and c.get("under28"):
        rects = {w["i"]: w["r"] for w in sn if Walk.shown(w)}
        edge = lambda r: r[1] <= vp[1] + 1 or r[1] + r[3] >= vp[1] + vp[3] - 1  # noqa: E731
        c["under28"] = [i for i in c["under28"] if not edge(rects.get(i, [0, 0, 0, 0]))]
        c["ok"] = not c["outside"] and not c["overlaps"] and not c["under28"]
    col = next((w["r"] for w in sn if w.get("i") == "b3_fleet_col" and Walk.shown(w)), None)
    pan = next((w["r"] for w in sn if w.get("i") == "b3_fleet_panel" and Walk.shown(w)), None)
    dx = round((col[0] + col[2] / 2) - (pan[0] + pan[2] / 2), 1) if col and pan else None
    W.check(f"{name}: pane numeric checks (no clipped/overlapping labels, controls >= 28 px, column centred)",
            c["ok"] and dx is not None and abs(dx) <= 1.5,
            checks_line(c) + f" column={round(col[2]) if col else None} column_dx={dx}"
            + (f" under28={c.get('under28')} outside={c.get('outside')} overlaps={c.get('overlaps')}" if not c["ok"] else ""))
    return c


def open_fleet(W: Walk) -> bool:
    if MODE == "phone":
        W.click("sidebar_toggle_hit")
        W.wait_shown("fleet_nav_hit", 8)
    W.mark()
    return W.click("fleet_nav_hit") and W.logged("b3.open.fleet", 6) and W.wait_shown("b3_fleet_panel", 10)


def walk(W: Walk) -> None:
    W.note("== 1. the sidebar footer's Fleet entry CLICK opens the pane; the lanes and the driver inventory load")
    W.check("fleet: Fleet entry CLICK -> the Fleet pane", open_fleet(W))
    W.check("fleet: the opening reads went out (lane source + inventory walk)",
            W.replay_saw("profile/sub_providers/list", 8) >= 1 and W.replay_saw("session/driver/get", 8) >= 1)
    W.check("fleet: control is ready -> the Start form shows", W.wait_shown("b3_fleet_form_title", 10))
    W.check("fleet: the walked dispatch is a union row 'Peer 1 · glm-4.6' (never the slug), grouped by its goal",
            W.wait(lambda: row_of(W, "Peer 1 · glm-4.6") is not None, 8) and W.text("b3_fleet_group_0") == "Goal goal_01"
            and not W.has_text("lint-sweep"),
            f"group={W.text('b3_fleet_group_0')!r} row0={W.text('b3_fleet_row_0_label')!r} status={status_of(W, 0)!r}")
    W.check("fleet: an inventory-only row reads 'Requested'", "Requested" in status_of(W, 0))
    seek(W, "b3_fleet_start_box")
    r = W.rect("b3_fleet_start_box")
    if r:
        W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
        time.sleep(1.5)
    W.check("fleet: Start is disabled with no lane and no brief (no button; a click sends NOTHING — no implicit lane)",
            r is not None and not W.visible("b3_fleet_start")
            and not replay_lines(W, "<- session/driver/acquire") and not replay_lines(W, "<- peer/dispatch"),
            f"start_box={r}")
    numeric(W, "open")
    W.shot(f"01-open-{MODE}")

    W.note("== 2. the lane picker lists ONLY the advertised lanes; no default")
    W.check("fleet: the Model picker CLICK opens the two advertised lanes",
            click_logged(W, "b3_fleet_model_tap", "b3.fleet.lane.toggle", lambda: bool(W.visible("b3_fleet_opt_1")))
            and W.text("b3_fleet_opt_0_label") == "lane-primary" and W.text("b3_fleet_opt_1_label") == "lane-review",
            f"options={[W.text('b3_fleet_opt_0_label'), W.text('b3_fleet_opt_1_label')]}")
    W.shot(f"02-picker-{MODE}")
    W.check("fleet: choosing lane-primary shows its provider/model",
            click_logged(W, "b3_fleet_opt_0", "b3.fleet.lane", lambda: W.text("b3_fleet_model_value") == "lane-primary")
            and W.text("b3_fleet_lane_title") == "openai/gpt-5.4", f"lane={W.text('b3_fleet_lane_title')!r}")

    W.note("== 3. Start = acquire (CAS) -> prepare -> ONE dispatch; the adopted row works, then waits for approval")
    type_into(W, "b3_fleet_brief", "Review the reconnect diff", "b3_fleet_brief_label")
    W.check("fleet: Start CLICK -> FleetStart job", click_logged(W, "b3_fleet_start", "b3.fleet.start"))
    W.check("fleet: the wire was acquire -> prepare -> ONE dispatch, the CAS on the walked revision 42",
            W.wait(lambda: len(replay_lines(W, "<- peer/dispatch")) == 1, 8)
            and len(replay_lines(W, "<- session/driver/acquire")) == 1 and len(replay_lines(W, "<- peer/prepare")) == 1
            and any("expected_revision=42" in l for l in replay_lines(W, "-> session/driver/acquire")),
            "; ".join(replay_lines(W, "(fleet sim)")[-3:]))
    # The union order: the walked dispatch is row 0, the adopted roster row 1.
    adopted = W.wait(lambda: seek(W, "b3_fleet_row_1_label", steps=20)
                     and W.text("b3_fleet_row_1_label") == "Peer 2 · gpt-5.4", 15)
    W.check("fleet: the adopted row 'Peer 2 · gpt-5.4' appears and the brief clears",
            adopted and seek(W, "b3_fleet_brief") and W.text("b3_fleet_brief") in ("", "Describe the task for the peer"),
            f"row1={W.text('b3_fleet_row_1_label')!r}")
    waiting = W.wait(lambda: "Waiting for your approval" in status_of(W, 1), 10)
    W.check("fleet: the adopted session's own frames drive it to 'Waiting for your approval'",
            waiting, f"status={status_of(W, 1)!r}")
    W.check("fleet: the live region announces it ('Peer 2 · gpt-5.4 is waiting for your approval')",
            seek(W, "b3_fleet_announce") and W.text("b3_fleet_announce") == "Peer 2 · gpt-5.4 is waiting for your approval",
            f"announce={W.text('b3_fleet_announce')!r}")
    r2 = row_of(W, "Peer 2") or 0
    numeric(W, "waiting")
    seek(W, f"b3_fleet_row_{r2}_approve")
    W.shot(f"03-waiting-{MODE}")

    W.note("== 4. Approve -> ONE peer/control approval_respond -> approval/decided -> Working")
    W.check("fleet: Approve CLICK -> peer/control(approval_respond) on the row's accepted operation + adopted turn",
            click_logged(W, f"b3_fleet_row_{r2}_approve", "b3.fleet.approve",
                         lambda: "Working" in status_of(W, row_of(W, "Peer 2") or 0), 10)
            and any("command=approval_respond" in l for l in replay_lines(W, "-> peer/control")),
            "; ".join(replay_lines(W, "-> peer/control")[-1:]))
    W.check("fleet: the row says 'Sent'", W.text(f"b3_fleet_row_{row_of(W, 'Peer 2') or 0}_note") == "Sent")

    W.note("== 5. Steer the working peer")
    r2 = row_of(W, "Peer 2") or 0
    type_into(W, f"b3_fleet_row_{r2}_steer", "Focus on the reconnect tests", f"b3_fleet_row_{r2}_title")
    W.check("fleet: Steer CLICK -> ONE peer/control(steer)",
            click_logged(W, f"b3_fleet_row_{r2}_steer_btn", "b3.fleet.steer",
                         lambda: any("command=steer" in l for l in replay_lines(W, "-> peer/control")), 10),
            "; ".join(replay_lines(W, "-> peer/control")[-1:]))

    W.note("== 6. a refused Start keeps the brief and the bounded label; its staging is a Failed row; the next Start mints a NEW id")
    seek(W, "b3_fleet_model_tap")
    W.click("b3_fleet_model_tap")
    W.wait_shown("b3_fleet_opt_1", 6)
    click_logged(W, "b3_fleet_opt_1", "b3.fleet.lane", lambda: W.text("b3_fleet_model_value") == "lane-review")
    type_into(W, "b3_fleet_brief", "Run the full test suite", "b3_fleet_brief_label")
    W.check("fleet: Start on lane-review -> the typed refusal -> 'Couldn't start: That model is not configured on this server'",
            click_logged(W, "b3_fleet_start", "b3.fleet.start", lambda: "not configured" in W.text("b3_fleet_error"), 10)
            and W.text("b3_fleet_brief") == "Run the full test suite",
            f"error={W.text('b3_fleet_error')!r} brief={W.text('b3_fleet_brief')!r}")
    W.check("fleet: the refused staging settles as a terminal 'Failed' row under Peers' Finished (1)",
            seek(W, "b3_fleet_finished_1_label") and W.text("b3_fleet_finished_1_label") == "Finished (1)",
            f"label={W.text('b3_fleet_finished_1_label')!r}")
    seek(W, "b3_fleet_error")
    W.shot(f"04-refused-{MODE}")
    seek(W, "b3_fleet_model_tap")
    W.click("b3_fleet_model_tap")
    W.wait_shown("b3_fleet_opt_0", 6)
    click_logged(W, "b3_fleet_opt_0", "b3.fleet.lane", lambda: W.text("b3_fleet_model_value") == "lane-primary")
    started = click_logged(W, "b3_fleet_start", "b3.fleet.start",
                           lambda: len(replay_lines(W, "-> peer/dispatch (fleet sim)")) == 2, 10)
    time.sleep(1.0)
    W.check("fleet: Start again (same brief, lane-primary) -> accepted 'Peer 4 · gpt-5.4'",
            started and seek(W, "b3_fleet_row_3_label") and W.text("b3_fleet_row_3_label") == "Peer 4 · gpt-5.4",
            f"row3={W.text('b3_fleet_row_3_label')!r}")
    ops = [re.search(r"operation_id=(\S+)", l).group(1) for l in replay_lines(W, "peer/dispatch") if "operation_id=" in l and "->" in l]
    W.check("fleet: three Starts = three DISTINCT operation ids on the wire", len(ops) == 3 and len(set(ops)) == 3, f"{ops}")

    W.note("== 6b. the peer asks a question: the row is an answer card; Answer -> ONE question_respond")
    r3 = 3
    asking = W.wait(lambda: "Waiting for your answer" in status_of(W, r3), 12)
    W.check("fleet: the adopted session's question folds into 'Waiting for your answer' with the question on the card",
            asking and seek(W, f"b3_fleet_row_{r3}_question")
            and W.text(f"b3_fleet_row_{r3}_question") == "Which color would you like to pick?",
            f"status={status_of(W, r3)!r} question={W.text(f'b3_fleet_row_{r3}_question')!r}")
    type_into(W, f"b3_fleet_row_{r3}_steer", "Blue", f"b3_fleet_row_{r3}_title")
    seek(W, f"b3_fleet_row_{r3}_answer")
    numeric(W, "answer")
    W.shot(f"04b-answer-{MODE}")
    W.check("fleet: Answer CLICK -> ONE peer/control(question_respond) on the row's operation + adopted turn; 'Sent'",
            click_logged(W, f"b3_fleet_row_{r3}_answer", "b3.fleet.answer",
                         lambda: any("command=question_respond" in l for l in replay_lines(W, "-> peer/control")), 10)
            and seek(W, f"b3_fleet_row_{r3}_note") and W.text(f"b3_fleet_row_{r3}_note") == "Sent",
            "; ".join(replay_lines(W, "-> peer/control")[-1:]))

    W.note("== 7. Stop -> ONE interrupt -> the row finishes under its group's Finished (n)")
    stopped = click_logged(W, f"b3_fleet_row_{r3}_stop", "b3.fleet.stop",
                           lambda: any("command=interrupt" in l for l in replay_lines(W, "-> peer/control")), 10)
    time.sleep(1.5)  # the pushed turn/error folds into the row
    g = 1  # the groups: Goal goal_01 (0), then Peers (1) — "Peers" last
    W.check("fleet: Stop CLICK -> peer/control(interrupt) -> turn/error -> 'Finished (2)' under Peers",
            stopped and seek(W, f"b3_fleet_finished_{g}_label") and W.text(f"b3_fleet_finished_{g}_label") == "Finished (2)",
            f"label={W.text(f'b3_fleet_finished_{g}_label')!r}")
    opened = click_logged(W, f"b3_fleet_finished_{g}", "b3.fleet.finished")
    time.sleep(0.6)
    W.check("fleet: the Finished (2) disclosure CLICK shows the stopped and the failed rows",
            opened and "Stopped" in status_of(W, r3) and "Failed" in status_of(W, 2)
            and seek(W, "b3_fleet_row_2_note") and W.text("b3_fleet_row_2_note") == "That model is not configured on this server",
            f"stopped={status_of(W, r3)!r} failed={status_of(W, 2)!r} reason={W.text('b3_fleet_row_2_note')!r}")
    numeric(W, "finished")
    seek(W, f"b3_fleet_finished_{g}")
    W.shot(f"05-finished-{MODE}")

    W.note("== 8. Advanced: the driver disclosure, Release seat -> re-walk -> Acquire seat")
    W.check("fleet: Advanced CLICK opens the session controller",
            click_logged(W, "b3_fleet_advanced", "b3.fleet.advanced", lambda: shown(W, "b3_fleet_disclosure_mode")))
    W.check("fleet: the disclosure names the external controller", "External" in W.text("b3_fleet_disclosure_mode"),
            f"{W.text('b3_fleet_disclosure_mode')!r}")
    W.check("fleet: the disclosure CLICK opens the read-only driver facts (recovery, driver, epoch, revision, lease)",
            click_logged(W, "b3_fleet_disclosure_toggle", "b3.fleet.console.disclosure", lambda: shown(W, "b3_fleet_disc_revision_v"))
            and W.text("b3_fleet_disc_recovery_v") == "No recovery pending" and W.text("b3_fleet_disc_epoch_v") == "8",
            f"revision={W.text('b3_fleet_disc_revision_v')!r} epoch={W.text('b3_fleet_disc_epoch_v')!r} "
            f"lease={W.text('b3_fleet_disc_lease_v')!r}")
    W.check("fleet: the session peers roster lists the rows", shown(W, "b3_fleet_console_roster"))
    seek(W, "b3_fleet_disclosure_mode")
    numeric(W, "advanced")
    W.shot(f"06-advanced-{MODE}")
    W.check("fleet: Release seat CLICK -> session/driver/release -> 'Acquire seat' appears",
            click_logged(W, "b3_fleet_console_release", "b3.fleet.console.release", lambda: shown(W, "b3_fleet_console_acquire"), 10)
            and len(replay_lines(W, "<- session/driver/release")) == 1)
    W.check("fleet: the release re-walks the inventory (web refreshControlInventory)",
            W.wait(lambda: len(replay_lines(W, "<- session/driver/get")) >= 2, 6), f"gets={len(replay_lines(W, '<- session/driver/get'))}")
    W.check("fleet: Acquire seat CLICK -> ONE acquire on the re-walked revision",
            click_logged(W, "b3_fleet_console_acquire", "b3.fleet.console.acquire",
                         lambda: len(replay_lines(W, "-> session/driver/acquire")) == 2, 10),
            "; ".join(replay_lines(W, "-> session/driver/acquire")))

    W.note("== 9. Peer gather: ONE peer/gather -> the synthesis as ONE ordinary turn")
    before = len(replay_lines(W, "<- turn/start"))
    W.check("fleet: Peer gather CLICK -> peer/gather -> turn/start -> 'Peer synthesis queued'",
            click_logged(W, "b3_fleet_gather", "b3.fleet.gather", lambda: "Peer synthesis queued" in W.text("b3_fleet_announce"), 10)
            and len(replay_lines(W, "<- peer/gather")) == 1 and len(replay_lines(W, "<- turn/start")) == before + 1)

    W.note("== 10. the control seat (PeerControlPanel): the master's live turn (the gather's synthesis) + the held seat's pending work")
    W.check("fleet: Back closes the pane and the composer returns",
            click_logged(W, "b3_fleet_back", "b3.close", lambda: not W.visible("b3_fleet_panel"))
            and W.wait(lambda: W.composer() is not None, 8))
    W.check("fleet: Fleet entry CLICK reopens the pane (the composer is hidden under it)",
            open_fleet(W) and W.composer() is None)
    if not seek(W, "b3_fleet_disclosure_mode"):
        click_logged(W, "b3_fleet_advanced", "b3.fleet.advanced", lambda: shown(W, "b3_fleet_disclosure_mode"))
    # The gather's synthesis is a chat turn: the composer's seat gate handed
    # this app's seat back first (`next: internal`, before its turn/start —
    # `releaseControlSeatForUserTurn`), so the console offers Acquire again.
    W.check("fleet: the synthesis turn handed the seat back first (release next=internal before its turn/start)",
            any("next=internal" in l for l in replay_lines(W, "-> session/driver/release"))
            and seek(W, "b3_fleet_console_acquire"),
            "; ".join(replay_lines(W, "-> session/driver/release")))
    W.check("fleet: Acquire seat CLICK -> ONE more acquire; the control seat mounts (held seat + pending work + live turn)",
            click_logged(W, "b3_fleet_console_acquire", "b3.fleet.console.acquire",
                         lambda: len(replay_lines(W, "-> session/driver/acquire")) == 3, 10)
            and W.wait(lambda: seek(W, "b3_fleet_seat_title"), 8))
    W.check("fleet: seat Steer CLICK -> ONE peer/control on the pending work + the master's live turn -> the receipt facts",
            click_logged(W, "b3_fleet_seat_cmd_2", "b3.fleet.console.seat", lambda: shown(W, "b3_fleet_seat_worker_v"), 10)
            and any("target_operation_id=synthetic-pending-op" in l for l in replay_lines(W, "-> peer/control")),
            f"worker={W.text('b3_fleet_seat_worker_v')!r} dup={W.text('b3_fleet_seat_dup_v')!r}")
    seek(W, "b3_fleet_seat_title")
    numeric(W, "seat")
    W.shot(f"07-seat-{MODE}")

    W.note("== 11. the wire: every click's method reached the replay server")
    for method, n in [("session/driver/acquire", 3), ("peer/prepare", 3), ("peer/dispatch", 3),
                      ("session/driver/release", 2), ("peer/gather", 1)]:
        got = len(replay_lines(W, f"<- {method} "))
        W.check(f"wire: {method} x{n}", got == n, f"replay log: {got}")
    W.check("wire: peer/control x5 (approve, steer, answer, stop, seat steer)", len(replay_lines(W, "<- peer/control ")) == 5,
            f"replay log: {len(replay_lines(W, '<- peer/control '))}")


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, port=PORT, replay_port=REPLAY, scenario="fleet"))
