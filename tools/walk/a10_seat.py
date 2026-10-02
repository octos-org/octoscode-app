#!/usr/bin/env python3
"""A10 — the peer-control seat by CLICK, where the web mounts it: the
Session settings pane's Advanced section (`SessionControlBar` as
`advancedChildren`, `App.tsx:3341-3349`; e2e `peer-control.spec.ts`). No
Stage-A board: the board-3 kit (A5 style) under A8's pane.

Session A (`replay_serve --scenario fleet --fleet-cold --revoke-file <f>`:
the FAITHFUL external-driver fixture, a COLD internal master, the app's
renew cadence shortened to 2 s by OCTOSCODE_SEAT_RENEW_MS):
1. the strip CLICK opens the pane; Advanced CLICK: "Peer controller" offers
   Acquire seat (opt-in, nothing auto-acquires), Release seat inert;
2. Acquire seat CLICK -> ONE acquire, CAS on the cold revision 0, under
   this app's stable driver id -> held: Release seat live, the re-walked
   disclosure names this app's peer (SELF);
3. the lease renews on the cadence (session/driver/renew with the fence);
4. Release seat CLICK -> ONE release next=external (parked) -> Acquire seat
   again; Acquire seat CLICK -> the CAS on the revision the release moved;
5. a chat send hands the held seat back FIRST (release next=internal before
   its turn/start); the console offers Acquire again;
6. Acquire seat CLICK during the live turn -> the seat panel ("Peer
   control": the acquire's pending work + the master's live turn) -> Steer
   CLICK -> ONE peer/control -> the receipt (Worker, "Newly applied");
7. another app takes the lease (the revoke file): the next renew is refused
   driver_fence_stale -> the seat is dropped without a frame, "Your control
   of this session expired" stays, Acquire seat is offered.
Session B (`--drop-method peer/control`, the e2e "no-method" variant): the
pane's Advanced mounts no seat and no console; zero acquire / control
frames.
usage: OCTOSCODE_APP_BIN=<host octosense> a10_seat.py <desktop|phone> <outdir>
"""
import pathlib
import sys
import time

from a10_lib import ROOT, Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/seat/{MODE}"
VP = "b3_scroll"
REVOKE = ROOT / "tmp" / "hs" / f"seat-revoke-{MODE}-{int(time.time())}"


def lines(W: Walk, needle: str) -> list[str]:
    if not W.replay_log or not W.replay_log.exists():
        return []
    return [l for l in W.replay_log.read_text().splitlines() if needle in l]


def seen(W: Walk, wid: str) -> bool:
    return bool(W.visible(wid)) or W.scroll_into(wid, VP)


def click(W: Walk, wid: str, expect, secs: float = 8.0) -> bool:
    time.sleep(0.4)
    ok = W.click_in(wid, VP) and W.wait(expect, secs)
    if not ok:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, VP) and W.wait(expect, secs)
    return ok


def numeric(W: Walk, name: str):
    W.wait(lambda: bool(W.visible("b3_dialog")), 6)
    c = dialog_checks(W.snap(), "b3_dialog", ("b3_fleet_", "b3_sc_", "b3_title", "b3_close"), viewport=VP)
    W.check(f"{name}: pane numeric checks", c["ok"], checks_line(c))


def open_pane(W: Walk) -> bool:
    # The strip mounts once the Session's status is read (later on a phone).
    W.wait(lambda: bool(W.visible("b3_strip_tap")), 20)
    ok = W.click("b3_strip_tap") and W.wait(lambda: W.text("b3_title") == "Session settings", 10)
    if ok and not W.visible("b3_sc_who") and not W.scroll_into("b3_sc_who", VP):
        ok = click(W, "b3_sc_adv", lambda: seen(W, "b3_sc_who"))
    return ok


def send_chat(W: Walk, text: str) -> bool:
    c = W.composer()
    if c is None:
        return False
    x, y, w, h = c["r"]
    W.note(f"CLICK composer r={c['r']}")
    W.click_xy(x + w / 2, y + h / 2)
    W.key("End")
    W.clear_field(40)
    W.type_text(text)
    W.key("Return")
    return W.wait(lambda: len(lines(W, "<- turn/start ")) >= 1, 8)


def walk_seat(W: Walk) -> None:
    W.note("== 1. the pane's Advanced: the console offers the seat")
    W.check("pane: the strip CLICK opens 'Session settings'; Advanced CLICK shows who controls it",
            open_pane(W) and W.text("b3_sc_who") == "This app", f"who={W.text('b3_sc_who')!r}")
    W.check("console: 'Peer controller' with Acquire seat offered, Release seat inert, no seat panel",
            seen(W, "b3_fleet_console_title") and seen(W, "b3_fleet_console_acquire")
            and bool(W.visible("b3_fleet_console_release_box")) and not W.visible("b3_fleet_console_release")
            and not W.visible("b3_fleet_seat_title"))
    W.check("console: nothing auto-acquires", not lines(W, "<- session/driver/acquire"))
    W.scroll_into("b3_fleet_console_title", VP)
    numeric(W, "offered")
    W.shot(f"01-offered-{MODE}")

    W.note("== 2. Acquire seat: ONE CAS acquire on the cold revision")
    W.check("acquire: Acquire seat CLICK -> ONE acquire expected_revision=0 -> Release seat live",
            click(W, "b3_fleet_console_acquire", lambda: len(lines(W, "-> session/driver/acquire")) == 1
                  and seen(W, "b3_fleet_console_release"))
            and "expected_revision=0" in lines(W, "-> session/driver/acquire")[0],
            "; ".join(lines(W, "-> session/driver/acquire")))
    own = W.wait(lambda: seen(W, "b3_sc_ownhold_head"), 6) and W.text("b3_sc_ownhold_head")
    W.check("held: the re-walked disclosure names this app's peer (SELF)",
            own == "A peer you started is using this session", repr(own))
    W.check("held: the console is bound to this app's stable driver id",
            seen(W, "b3_fleet_console_bound") and W.text("b3_fleet_console_bound").startswith("Bound to octoscode-native:"),
            repr(W.text("b3_fleet_console_bound")))
    W.scroll_into("b3_fleet_console_title", VP)
    numeric(W, "held")
    W.shot(f"02-held-{MODE}")

    W.note("== 3. the lease renews on the cadence")
    W.check("renew: session/driver/renew x2 while held (2 s cadence in this walk; 45 s in the product)",
            W.wait(lambda: len(lines(W, "<- session/driver/renew ")) >= 2, 9),
            f"renews={len(lines(W, '<- session/driver/renew '))}")

    W.note("== 4. Release seat parks it; Acquire seat takes it again")
    W.check("release: Release seat CLICK -> ONE release next=external -> Acquire seat offered",
            click(W, "b3_fleet_console_release", lambda: len(lines(W, "-> session/driver/release")) == 1
                  and seen(W, "b3_fleet_console_acquire"))
            and "next=external" in lines(W, "-> session/driver/release")[0],
            "; ".join(lines(W, "-> session/driver/release")))
    W.shot(f"03-released-{MODE}")
    W.check("acquire: Acquire seat CLICK -> the CAS on the revision the release moved (2)",
            click(W, "b3_fleet_console_acquire", lambda: len(lines(W, "-> session/driver/acquire")) == 2
                  and seen(W, "b3_fleet_console_release"))
            and "expected_revision=2" in lines(W, "-> session/driver/acquire")[1])

    W.note("== 5. a chat send hands the held seat back first")
    W.check("pane: the close CLICK", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))
    W.check("chat: a prompt CLICK + Return -> turn/start", send_chat(W, "summarise the open peers"))
    log = W.replay_log.read_text().splitlines() if W.replay_log else []
    rel = [i for i, l in enumerate(log) if "-> session/driver/release" in l and "next=internal" in l]
    start = [i for i, l in enumerate(log) if "<- turn/start " in l]
    W.check("chat: the seat went back (release next=internal) BEFORE the one turn/start",
            bool(rel) and bool(start) and rel[0] < start[0] and len(start) == 1,
            f"release@{rel} turn/start@{start}")
    W.check("pane: reopened, the console offers Acquire seat again (handed back)",
            open_pane(W) and seen(W, "b3_fleet_console_acquire"))

    W.note("== 6. the seat panel: the acquire's pending work + the live turn")
    W.check("acquire: Acquire seat CLICK during the live turn -> the seat panel 'Peer control'",
            click(W, "b3_fleet_console_acquire", lambda: len(lines(W, "-> session/driver/acquire")) == 3
                  and seen(W, "b3_fleet_seat_title"), 10))
    W.check("seat: Steer CLICK -> ONE peer/control on the pending work + the live turn -> Worker + 'Newly applied'",
            click(W, "b3_fleet_seat_cmd_2", lambda: seen(W, "b3_fleet_seat_worker_v"), 10)
            and len(lines(W, "-> peer/control")) == 1
            and "target_operation_id=synthetic-pending-op" in lines(W, "-> peer/control")[0]
            and "command=steer" in lines(W, "-> peer/control")[0]
            and W.text("b3_fleet_seat_dup_v") == "Newly applied",
            f"worker={W.text('b3_fleet_seat_worker_v')!r} dup={W.text('b3_fleet_seat_dup_v')!r} "
            f"{'; '.join(lines(W, '-> peer/control'))}")
    W.scroll_into("b3_fleet_seat_title", VP)
    numeric(W, "seat")
    W.shot(f"04-seat-{MODE}")

    W.note("== 7. another app takes the lease: the seat is dropped, the label stays")
    REVOKE.write_text("revoke")
    W.check("expired: the next renew is refused driver_fence_stale -> 'Your control of this session expired'",
            W.wait(lambda: bool(lines(W, "-> session/driver/renew REFUSED")), 9)
            and W.wait(lambda: seen(W, "b3_fleet_console_expired"), 6)
            and W.text("b3_fleet_console_expired") == "Your control of this session expired",
            f"{'; '.join(lines(W, 'renew REFUSED'))}")
    releases = len(lines(W, "<- session/driver/release "))
    W.check("expired: dropped without a frame; no seat panel; Acquire seat offered",
            not W.visible("b3_fleet_seat_title") and seen(W, "b3_fleet_console_acquire")
            and len(lines(W, "<- session/driver/release ")) == releases)
    holder = W.wait(lambda: seen(W, "b3_sc_holder_head"), 8) and W.text("b3_sc_holder_head")
    W.check("expired: the re-walk names the new holder — 'Another app is using this session' + Resume chat",
            holder == "Another app is using this session" and seen(W, "b3_sc_resume"), repr(holder))
    renews = len(lines(W, "<- session/driver/renew "))
    time.sleep(4.5)
    W.check("expired: never retried with the dead fence (no renew after the refusal)",
            len(lines(W, "<- session/driver/renew ")) == renews, f"renews {renews} -> {len(lines(W, '<- session/driver/renew '))}")
    W.scroll_into("b3_fleet_console_expired", VP)
    numeric(W, "expired")
    W.shot(f"05-expired-{MODE}")
    W.check("pane: the close CLICK", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))


def walk_no_method(W: Walk) -> None:
    W.note("== the e2e no-method variant: peer/control unadvertised")
    W.check("pane: the strip CLICK opens 'Session settings'; Advanced CLICK shows who controls it",
            open_pane(W) and seen(W, "b3_sc_who"))
    time.sleep(1.0)
    W.check("no-method: no seat panel and no console", not seen(W, "b3_fleet_console_title") and not seen(W, "b3_fleet_seat_title"))
    W.check("no-method: zero acquire / control frames",
            not lines(W, "<- session/driver/acquire") and not lines(W, "<- peer/control"))
    W.scroll_into("b3_sc_who", VP)
    numeric(W, "no-method")
    W.shot(f"06-no-method-{MODE}")
    W.check("pane: the close CLICK", W.click("b3_close") and W.wait_shown("b3_dialog", 6, gone=True))


if __name__ == "__main__":
    env = {"OCTOSCODE_SEAT_RENEW_MS": "2000"}
    a = run_session(walk_seat, mode=MODE, outdir=OUT, scenario="fleet", env=env,
                    replay_args=["--fleet-cold", "--revoke-file", str(REVOKE)])
    b = run_session(walk_no_method, mode=MODE, outdir=str(pathlib.Path(OUT).parent / f"{MODE}-no-method"),
                    scenario="fleet", env=env, replay_args=["--fleet-cold", "--drop-method", "peer/control"])
    try:
        REVOKE.unlink()
    except FileNotFoundError:
        pass
    sys.exit(a or b)
