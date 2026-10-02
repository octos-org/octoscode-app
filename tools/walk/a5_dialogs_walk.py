#!/usr/bin/env python3
"""A5's dialog click walk, committed with checks (A11 port of the 22-step walk
whose log is docs/ux/a5-dialogs/walk/walk.log): every dialog is opened from
the REAL UI — a palette row CLICK, the palette by KEYBOARD (filter + Enter), a
typed command, or Settings > Model — and every control is CLICKED. Each step
asserts the app's own receipt (its log line) and, where it reaches the
server, the replay server's wire log (`<- method`).

  python3 tools/walk/a5_dialogs_walk.py <port> <desktop|phone> <replay-log>

The app runs hidden against `replay_serve <rport> --scenario screens`
(recorded r1/r2/r3/r6/c24b replies), with isolated state; the walk
aggregator (tools/walk/native.py) launches both from the WALK literal below
and always stops them. Exit 0 iff every check passed.
"""
import json
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from walk_env import App  # noqa: E402

WALK = {
    "name": "a5_dialogs",
    "title": "the nine palette dialogs: Models, Context, Skills, Goal, Loops, Monitors, Fleet, Tasks, Code review",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "screens"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "dsflash"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}", "{mode}", "{fixture_log}"]}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {
        68: ["palette: '/' lists", "/compact by KEYBOARD"],
        87: {"checks": ["Settings > Model > Manage models"],
             "partial": "the provider editor itself is a2_board1's (Settings > Model > Model providers · Edit)"},
        98: {"checks": ["/goal by CLICK", "goal: Pause", "goal: Stop", "goal: Clear goal", "goal: Set goal"],
             "partial": "pause/stop/clear/set walked; resume and the newer-notification ordering are not staged"},
        # A11: row 100's creations are A10's cadence form (tools/walk/a10_autonomy.py).
        100: {"checks": ["/loop by CLICK", "loops: + New loop opens the form"],
              "partial": "the dialog and its create form open by CLICK (the creations are a10_autonomy's)"},
        102: {"checks": ["/monitor by CLICK", "monitors: Pause", "monitors: Resume", "monitors: Delete"],
              "partial": "list, pause, resume, delete (create is a10_autonomy's)"},
        103: ["loops: Fire now"],
    },
}

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8415
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
REPLAY_LOG = pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None
PHONE = MODE == "phone"
PLACEHOLDER = "Ask Octos anything"
app = App(PORT)
RESULTS = []


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok)))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
    return bool(ok)


def wire(method):
    """How many `<- method` requests reached the replay server."""
    if not REPLAY_LOG or not REPLAY_LOG.exists():
        return 0
    return sum(1 for l in REPLAY_LOG.read_text().splitlines() if f"<- {method} " in l + " ")


def receipts(wait=1.2):
    time.sleep(wait)
    return [l for l in app.logs() if "[octoscode]" in l]


def has(lines, needle):
    return any(needle in l for l in lines)


def composer_text():
    t = app.text("i0_composer_0")
    return "" if t == PLACEHOLDER else t


# The phone shell's emulated soft keyboard is drawn by the shell (not in
# /snap) over the lower screen once a field has focus — the composer then sits
# UNDER it and a tap on the composer's rect types a key ('c'). Its hide chevron
# is at the top-right of the keyboard (measured in the 360x780 frame: the bar
# from y 549, the chevron at ~375,571).
KB_CHEVRON = (375, 571)


def keyboard_up():
    if not PHONE:
        return False
    data = app.png()
    if not data:
        return False
    try:
        import io
        from PIL import Image
        im = Image.open(io.BytesIO(data)).convert("RGB")
    except ImportError:
        return True  # cannot tell: hiding a hidden keyboard taps blank ground
    k = im.width / 402.0  # the phone shell frame is 402 pt wide
    cx, cy = KB_CHEVRON
    return any(sum(im.getpixel((int(x * k), int(y * k)))) < 200
               for x in range(cx - 10, cx + 11, 2) for y in range(cy - 7, cy + 8, 2))


def tap_composer():
    """Focus the composer — hiding the phone keyboard first if it covers it."""
    if keyboard_up():
        app.click_xy(*KB_CHEVRON)
        time.sleep(0.6)
    app.click("i0_composer_0")


def clear_composer():
    """The composer may hold a restored prompt: End, then one Backspace per
    character (the instrument's synthetic Cmd+A never reaches select-all)."""
    n = len(composer_text())
    if n:
        tap_composer()
        app.key("end")
        for _ in range(n + 1):
            app.key("backspace")


def palette(query):
    """'/' in the empty composer opens the palette; type the filter. Waits
    for the rows (the palette re-opens a beat after an emptied composer)."""
    clear_composer()
    time.sleep(0.4)
    tap_composer()
    app.type("/")
    app.wait(lambda: app.find("palette_row_name"), timeout=4)
    if query:
        app.type(query)
    app.wait(lambda: any((w.get("t") or "").startswith("/" + query) for w in app.all("palette_row_name")), timeout=4)
    return [w.get("t") for w in app.all("palette_row_name")]


def row(text):
    for w in app.all("palette_row_name"):
        if w.get("t") == text:
            x, y, ww, hh = w["r"]
            app.click_xy(x + ww / 2, y + hh / 2)
            return True
    return False


def close_dialog():
    if not app.click("dialog_close") and not PHONE:
        app.key("Escape")
    app.wait(lambda: not app.find("dialog_close"), timeout=4)


def dismiss_palette():
    """Escape closes the palette on a desktop; a phone has no Escape key (A6's
    walk notes the same), so there the emptied composer closes it."""
    if PHONE:
        clear_composer()
    else:
        app.key("Escape")
    time.sleep(0.5)


def open_dialog(query, cmd, dialog):
    rows = palette(query)
    app.logs()
    clicked = row(cmd)
    lines = receipts(2.0)
    return check(f"{cmd} by CLICK opens the {dialog} dialog",
                 clicked and has(lines, f"palette run {cmd} -> dialog.open.") and has(lines, f"dialog {dialog} mounted")
                 and app.find("dialog_close") is not None,
                 f"rows {rows}")


def tap(wid, needle, name, wait=1.5, nth=0):
    app.logs()
    clicked = app.click(wid, nth)
    lines = receipts(wait)
    return check(name, clicked and has(lines, needle), f"{wid} clicked={clicked}")


def main():
    if not app.wait(lambda: app.find("i0_composer_0"), timeout=30):
        check("the composer is up", False)
        return 1
    app.logs()

    # 1. the palette lists the dialog commands (live, capability-gated)
    rows = palette("")
    check("palette: '/' lists the dialog commands", "/model" in rows and "/compact" in rows, f"{rows}")
    dismiss_palette()

    # 2. /model by CLICK; Test route and Discover models by CLICK
    if open_dialog("mo", "/model", "models"):
        tap("dlg_models_btn_test_control", "models.test_route done", "models: Test route by CLICK")
        tap("dlg_models_btn_discover_control", "models.discover done", "models: Discover models by CLICK")
        close_dialog()

    # 3. /compact by KEYBOARD (filter + Enter); Heuristic; Compact now -> confirm -> Confirm
    palette("comp")
    app.logs()
    app.key("Return")
    lines = receipts(2.0)
    if check("/compact by KEYBOARD (filter + Enter) opens the Context dialog",
             has(lines, "palette run /compact -> dialog.open.context") and has(lines, "dialog context mounted")):
        tap("dlg_context_seg_heur_hit", "context.mode.heuristic done", "context: Heuristic mode by CLICK")
        n0 = wire("session/compact")
        tap("dlg_context_btn_compact_control", "dialog confirm asked: context.compact_now",
            "context: Compact now asks first (the confirm card)", wait=1.0)
        app.logs()
        clicked = app.click("dlg_context_cf_confirm_control")
        lines = receipts(1.5)
        check("context: Confirm compacts (session/compact on the wire)",
              clicked and has(lines, "dialog confirmed: context.compact_now") and wire("session/compact") == n0 + 1,
              f"session/compact x{wire('session/compact') - n0}")
        close_dialog()

    # 4. /skills by CLICK; search the registry (Enter)
    if open_dialog("sk", "/skills", "skills"):
        app.click("dlg_skills_skills_query")
        app.type("lint")
        app.logs()
        app.key("Return")
        lines = receipts(2.0)
        check("skills: the registry search runs on Enter", has(lines, 'skills search "lint"'))
        close_dialog()

    # 5. /goal by CLICK; Pause, Stop, Clear goal; then the Set goal form
    if open_dialog("go", "/goal", "goal"):
        tap("dlg_goal_pause_btn_control", 'Transition("paused")', "goal: Pause by CLICK")
        tap("dlg_goal_stop_btn_control", 'Transition("complete")', "goal: Stop by CLICK")
        tap("dlg_goal_clear_goal_control", "ClearGoal", "goal: Clear goal by CLICK")
        tap("dlg_goal_pause_btn_control", "dialog form opened: goal.set", "goal: Set goal opens the goal form", wait=1.0)
        app.click("dlg_goal_gf_objective")
        app.type("Ship the native dialogs")
        app.click("dlg_goal_gf_budget")
        app.type("100000")
        tap("dlg_goal_ff_submit_control", 'SetGoal { objective: "Ship the native dialogs", token_budget: Some(100000) }',
            "goal: Set goal submits the objective + budget")
        close_dialog()

    # 6. /loop by CLICK; row 1 pause, resume, fire now, delete
    if open_dialog("lo", "/loop", "loops"):
        tap("dlg_loops_loop_1_pause_hit", 'LoopPause("loop_01")', "loops: Pause by CLICK")
        tap("dlg_loops_loop_1_play_hit", 'LoopResume("loop_01")', "loops: Resume by CLICK")
        n0 = wire("loop/fire_now")
        tap("dlg_loops_loop_1_play_hit", 'LoopFireNow("loop_01")', "loops: Fire now by CLICK (one typed loop/fire_now)")
        check("loops: Fire now sent exactly one loop/fire_now", wire("loop/fire_now") == n0 + 1,
              f"x{wire('loop/fire_now') - n0}")
        tap("dlg_loops_loop_1_trash_hit", 'LoopDelete("loop_01")', "loops: Delete by CLICK")
        close_dialog()

    # 6b. + New loop opens the form. A11: A10 replaced A5's prompt + interval
    # form with the web's cadence form (Maintenance by default, Self-paced,
    # Fixed interval); its create paths are tools/walk/a10_autonomy.py's
    # (an empty Create under the default Maintenance cadence is a valid
    # maintenance loop now, not a refusal).
    if open_dialog("lo", "/loop", "loops"):
        tap("dlg_loops_new_loop_control", "dialog form opened: loop.create", "loops: + New loop opens the form", wait=1.0)
        close_dialog()

    # 7. /monitor by CLICK; pause, resume, delete
    if open_dialog("mon", "/monitor", "monitors"):
        tap("dlg_monitors_mon_1_pause_hit", 'MonitorPause("monitor_01")', "monitors: Pause by CLICK")
        tap("dlg_monitors_mon_1_pause_hit", 'MonitorResume("monitor_01")', "monitors: Resume by CLICK")
        tap("dlg_monitors_mon_1_trash_hit", 'MonitorDelete("monitor_01")', "monitors: Delete by CLICK")
        close_dialog()

    # 8. /peer by CLICK -> the Fleet dialog. A11: A10 moved the peer row
    # actions (Steer / Approve / Stop) to Fleet's pane and its seat; they are
    # tools/walk/a10_fleet.py's and a10_seat.py's.
    if open_dialog("pe", "/peer", "fleet"):
        close_dialog()

    # 9. /ps by CLICK -> Tasks; Cancel the running task
    if open_dialog("ps", "/ps", "tasks"):
        tap("dlg_tasks_run_r0_cancel_control", "fleet action -> task/cancel", "tasks: Cancel by CLICK (task/cancel)")
        close_dialog()

    # 10. /review by CLICK -> Code review. A11: A10 rebuilt the dialog on the
    # web's NativeReviewDialog (instructions field; Start admits a Session
    # turn and closes it) — tools/walk/a10_review.py walks Start.
    if open_dialog("rev", "/review", "review"):
        close_dialog()

    # 11-13. typed commands run LOCALLY: never sent to the model
    # On a desktop Escape first dismisses the palette, so Return submits the
    # composer (ComposerSubmit); a phone has no Escape: Return runs the
    # palette's row — either way the command runs locally, never sent.
    turns0 = wire("turn/start")
    clear_composer()
    tap_composer()
    app.type("/model")
    if not PHONE:
        app.key("Escape")
    app.logs()
    app.key("Return")
    lines = receipts(2.0)
    check("a typed /model runs locally (the Models dialog)",
          (PHONE or has(lines, "command /model: queued to run locally"))
          and has(lines, "palette run /model -> dialog.open.models"))
    close_dialog()
    clear_composer()
    tap_composer()
    app.type("/goal ship it")
    app.logs()
    app.key("Return")
    lines = receipts(2.0)
    check("a command with arguments is reported, never run", has(lines, "palette run /goal: arguments reported"))
    clear_composer()
    tap_composer()
    app.type("/stop")
    if not PHONE:
        app.key("Escape")
    app.logs()
    app.key("Return")
    lines = receipts(2.0)
    check("/stop is the interrupt intent", has(lines, "palette run /stop -> turn.interrupt"))
    check("typed commands never reached the model (no turn/start)", wire("turn/start") == turns0,
          f"x{wire('turn/start') - turns0}")

    # 14. Settings > Model > Manage models… -> the Models dialog over Settings
    clear_composer()
    app.logs()
    opened = app.click_until("settings_open_hit", lambda: app.find("settings_drawer"))
    cell = "rl_hit" if PHONE else "nv_hit"
    app.click_until(cell, lambda: app.find("set_models_manage"), nth=2)
    app.click("set_models_manage")
    lines = receipts(2.0)
    check("Settings > Model > Manage models… opens the Models dialog over Settings",
          opened and has(lines, "dialog.open.models") and app.find("dialog_close") is not None)
    close_dialog()
    app.click_until("set_back" if PHONE else "settings_close", lambda: not app.find("settings_drawer"))

    failed = [n for n, ok in RESULTS if not ok]
    print(f"== WALK a5_dialogs {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
