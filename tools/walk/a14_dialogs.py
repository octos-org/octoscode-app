#!/usr/bin/env python3
"""A14 — the A5 dialog family on the board-3 kit, measured by click walk.

Every dialog of the family (Tasks /ps, Skills /skills, Goal /goal, Loops
/loop, Monitors /monitor, Models /model, Code review /review, Context
/compact (alias /context), the Fleet slice /peer) is opened by a palette CLICK against
`replay_serve --scenario screens` (the judge tour's scenario), captured, and
measured from /snap:

* frame: its width (the board-3 range: 480-800 on the desktop, 328 on a
  360 px phone), its centring in the module view, the content margins;
* header: the title's line height (17 px semibold -> 21), the 28 px close;
* controls: every hit target >= 28 px, the tallest pill (<= 36: no 40-55 px
  phone pills); no label outside the frame, no two labels overlapping;
* the judge's defects: the Tasks console has no caret under "Waiting for
  output…" (a caret, when there is output, sits after the last line); the
  Skills search glyph and query share one centre line (<= 1.5 px);
* the create / confirm cards (+ New loop, + New monitor, Review
  installation) and the Fleet PANE's empty state (centred, never a word at
  the header's far right).

usage: OCTOSCODE_APP_BIN=<host octosense> [A10_PORT=8427 A10_REPLAY_PORT=8439] \
       a14_dialogs.py <desktop|phone> <outdir> [--before] [--seeded]
`--before` measures an older build: the A14-only checks are reported, not
gated. `--seeded` opens the dialogs on the A5 reference-board fixture instead
(OCTOSCODE_SYNTHETIC_LIVE=1 OCTOSCODE_DIALOG_SEED=1, no server): populated
states — a running task WITH output (the caret after its last line), three
peers, installed + registry skills, a goal with a budget, three providers.
"""
import json
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a14/after-{MODE}"
BEFORE = "--before" in sys.argv
SEEDED = "--seeded" in sys.argv
VP = "dialog_scroll"
W: Walk = None  # set by walk()
ROWS: list[dict] = []

DIALOGS = [
    # (dialog id, palette query, palette row, a widget that proves it is up)
    ("tasks", "ps", "/ps", "dlg_tasks_t_title"),
    ("skills", "ski", "/skills", "dlg_skills_skills_warning"),
    ("goal", "goa", "/goal", "dlg_goal_t_goal"),
    ("loops", "loo", "/loop", "dlg_loops_t_title"),
    ("monitors", "moni", "/monitor", "dlg_monitors_t_title"),
    ("models", "mo", "/model", "dlg_models_t_title"),
    ("review", "revi", "/review", "dlg_review_t_status"),
    ("context", "compa", "/compact", "dlg_context_t_title"),
    ("fleet", "pee", "/peer", "dlg_fleet_t_title"),
]


def gate(name: str, ok: bool, detail: str = "") -> bool:
    """An A14 check: gated on the new build, reported on --before."""
    if BEFORE:
        W.note(("INFO-PASS " if ok else "INFO-FAIL ") + name + (f" — {detail}" if detail else ""))
        return ok
    return W.check(name, ok, detail)


def metrics(sn, prefix: str) -> dict:
    c = dialog_checks(sn, "dialog_frame", (prefix, "dialog_close"), viewport=VP)
    shown = [w for w in sn if Walk.shown(w)]
    mine = [w for w in shown if str(w.get("i", "")).startswith(prefix)]
    title = next((w for w in mine if w["i"].endswith("_t_title") or w["i"].endswith("_cf_title")), None)
    close = next((w for w in shown if w.get("i") == "dialog_close"), None)
    taps = [w for w in mine + ([close] if close else []) if w.get("ty") in ("Button", "DesignNativeButton")]
    # The pills: a visible surface/box that a tap covers exactly.
    pills = [w for w in mine if (w["i"].endswith("_surface") or w["i"].endswith("_box"))
             and any(abs(t["r"][1] - w["r"][1]) < 1 and abs(t["r"][3] - w["r"][3]) < 1 for t in taps)]
    c["title_h"] = title["r"][3] if title else None
    c["close"] = close["r"][2:] if close else None
    c["min_hit"] = min((min(t["r"][2], t["r"][3]) for t in taps), default=None)
    c["max_pill_h"] = max((p["r"][3] for p in pills), default=None)
    return c


def row_line(c: dict) -> str:
    if "frame" not in c:
        return c.get("why", "no frame")
    return (checks_line(c) + f" title_h={c['title_h']} close={c['close']} min_hit={c['min_hit']}"
            f" max_pill_h={c['max_pill_h']}")


def record(dialog: str, state: str, c: dict, png: str) -> None:
    ROWS.append({"dialog": dialog, "state": state, "mode": MODE, "png": png, **{k: c.get(k) for k in (
        "frame", "centre_dx", "margins", "labels", "outside", "overlaps", "controls", "under28", "title_h",
        "close", "min_hit", "max_pill_h", "ok")}})


def judge(dialog: str, state: str, prefix: str) -> dict:
    W.wait(lambda: bool(W.visible("dialog_frame")), 8)
    time.sleep(0.6)
    sn = W.snap()
    c = metrics(sn, prefix)
    W.check(f"{dialog} [{state}]: no label outside the frame, none overlapping, every hit >= 28",
            c.get("ok", False), row_line(c))
    if "frame" in c:
        fr = c["frame"]
        if MODE == "desktop":
            gate(f"{dialog} [{state}]: the board-3 width (480-800), centred", 470 <= fr[2] <= 800
                 and abs(c["centre_dx"] or 0) <= 1.5, f"w={fr[2]} centre_dx={c['centre_dx']}")
        else:
            gate(f"{dialog} [{state}]: the phone card (328 wide, centred)", abs(fr[2] - 328) <= 1.5
                 and abs(c["centre_dx"] or 0) <= 1.5, f"w={fr[2]} centre_dx={c['centre_dx']}")
        gate(f"{dialog} [{state}]: the board-3 header (21 px title line, 28 px close)",
             c["title_h"] is not None and abs(c["title_h"] - 21) <= 1.5 and c["close"] == [28, 28],
             f"title_h={c['title_h']} close={c['close']}")
        gate(f"{dialog} [{state}]: compact pills (<= 36 px)", (c["max_pill_h"] or 0) <= 36,
             f"max_pill_h={c['max_pill_h']}")
    name = f"{dialog}-{state}-{MODE}"
    png = W.shot(name)
    record(dialog, state, c, str(png.name))
    return c


def close_dialog() -> None:
    if W.visible("dialog_close"):
        W.click("dialog_close")
        W.wait_shown("dialog_frame", 6, gone=True)
    time.sleep(0.4)


def open_dialog(dialog: str, q: str, row: str, proof: str) -> bool:
    for attempt in range(2):
        if W.palette_run(q, row) and W.wait(lambda: bool(W.visible("dialog_frame")), 10):
            W.wait_shown(proof, 6)
            return True
        W.note(f"RETRY open {dialog}")
        close_dialog()
    return False


def tasks_checks() -> None:
    sn = W.snap()
    waiting = any("Waiting for output" in (w.get("t") or "") for w in sn if Walk.shown(w)
                  and str(w.get("i", "")).startswith("dlg_tasks_run_r"))
    caret = W.rect("dlg_tasks_run_r0_cursor", sn=sn)
    if waiting:
        gate("tasks: no caret under 'Waiting for output…'", caret is None, f"caret={caret}")
    elif caret:
        last = max((w for w in sn if Walk.shown(w) and str(w.get("i", "")).startswith("dlg_tasks_run_r0_log")),
                   key=lambda w: w["r"][1], default=None)
        gate("tasks: the caret follows the last output line", last is not None
             and abs(caret[0] - (last["r"][0] + last["r"][2])) <= 4 and abs(caret[1] - last["r"][1]) <= 3,
             f"caret={caret} last={last and last['r']}")


def skills_checks() -> None:
    sn = W.snap()
    icon, query = W.rect("dlg_skills_skills_query_icon", sn=sn), W.rect("dlg_skills_skills_query", sn=sn)
    if icon and query:
        dy = (icon[1] + icon[3] / 2) - (query[1] + query[3] / 2)
        gate("skills: the search glyph and the query share one centre line", abs(dy) <= 1.5,
             f"glyph={icon} query={query} dy={dy:.1f}")
    else:
        gate("skills: the search glyph and the query share one centre line", False, f"glyph={icon} query={query}")


def type_into(wid: str, text: str) -> None:
    W.scroll_into(wid, VP)
    r = W.rect(wid)
    if not r:
        W.note(f"no field {wid}")
        return
    W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.clear_field(30)
    W.type_text(text)


def click_logged(wid: str, needle: str, expect, secs: float = 8.0) -> bool:
    time.sleep(0.4)
    W.mark()
    ok = W.click_in(wid, VP)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = W.click_in(wid, VP)
        logged = W.logged(needle, secs / 2) if ok else False
    return ok and logged and W.wait(expect, secs)


def fleet_pane() -> None:
    W.note("== the Fleet pane (sidebar → Fleet): its empty state")
    if MODE == "phone":
        if W.click("sidebar_toggle_hit"):
            time.sleep(1.0)
    ok = W.click("fleet_nav_hit") and W.wait(lambda: bool(W.visible("b3_fleet_col")), 10)
    W.check("fleet pane: the sidebar's Fleet CLICK opens it", ok)
    time.sleep(0.8)
    sn = W.snap()
    col, title, none = W.rect("b3_fleet_col", sn=sn), W.rect("b3_title", sn=sn), W.rect("b3_fleet_none", sn=sn)
    if col and none and title:
        dx = (none[0] + none[2] / 2) - (col[0] + col[2] / 2)
        gate("fleet pane: 'No peers yet' is a centred empty state under the header, not a header word",
             abs(dx) <= 2 and none[1] > title[1] + title[3] + 20, f"line={none} col={col} title={title} dx={dx:.1f}")
        hint = W.rect("b3_fleet_unsupported", sn=sn) or W.rect("b3_fleet_notready", sn=sn)
        if hint:
            hdx = (hint[0] + hint[2] / 2) - (col[0] + col[2] / 2)
            gate("fleet pane: the start-unavailable note is the empty state's centred hint",
                 abs(hdx) <= 2 and hint[1] > none[1], f"hint={hint} dx={hdx:.1f}")
    else:
        gate("fleet pane: 'No peers yet' drawn", False, f"col={col} title={title} none={none}")
    W.shot(f"fleet-pane-empty-{MODE}")
    ROWS.append({"dialog": "fleet-pane", "state": "empty", "mode": MODE, "png": f"fleet-pane-empty-{MODE}.png",
                 "line": none, "col": col})
    if W.visible("b3_fleet_back"):
        W.click("b3_fleet_back")
        W.wait_shown("b3_fleet_col", 6, gone=True)


def walk(w: Walk) -> None:
    global W
    W = w
    for dialog, q, row, proof in DIALOGS:
        W.note(f"== {dialog}: {row} palette CLICK")
        ok = open_dialog(dialog, q, row, proof)
        W.check(f"{dialog}: the {row} palette row CLICK opens the dialog", ok)
        if not ok:
            continue
        judge(dialog, "seeded" if SEEDED else "open", f"dlg_{dialog}_")
        if dialog == "tasks":
            tasks_checks()
        if SEEDED:
            close_dialog()
            continue
        if dialog == "skills":
            skills_checks()
            # Review installation -> the confirm card.
            if W.visible("dlg_skills_src_repo") or W.scroll_into("dlg_skills_src_repo", VP):
                type_into("dlg_skills_src_repo", "octos-org/review-kit")
                W.dismiss_keyboard("dlg_skills_t_title")
                if click_logged("dlg_skills_src_review_control", "dialog confirm asked: skills.install_source",
                                lambda: bool(W.visible("dlg_skills_cf_detail"))):
                    judge(dialog, "confirm", "dlg_skills_")
                    if W.visible("dlg_skills_cf_cancel_control"):
                        W.click("dlg_skills_cf_cancel_control")
                        time.sleep(0.8)
        if dialog == "loops":
            if click_logged("dlg_loops_new_loop_control", "dialog form opened: loop.create",
                            lambda: bool(W.visible("dlg_loops_cf_title"))):
                judge(dialog, "form", "dlg_loops_")
                W.click("dlg_loops_ff_cancel_control")
                time.sleep(0.8)
        if dialog == "monitors":
            if click_logged("dlg_monitors_new_monitor_control", "dialog form opened: monitor.create",
                            lambda: bool(W.visible("dlg_monitors_cf_title"))):
                judge(dialog, "form", "dlg_monitors_")
                W.click("dlg_monitors_ff_cancel_control")
                time.sleep(0.8)
        close_dialog()
    if not SEEDED:
        fleet_pane()
    (W.out / "rows.json").write_text(json.dumps(ROWS, indent=1))


if __name__ == "__main__":
    if SEEDED:
        sys.exit(run_session(walk, mode=MODE, outdir=OUT, replay_port=None,
                             env={"OCTOSCODE_SYNTHETIC_LIVE": "1", "OCTOSCODE_DIALOG_SEED": "1"}))
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="screens"))
