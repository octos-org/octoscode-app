#!/usr/bin/env python3
"""A10 — the autonomy dialogs' create forms and alert line, by CLICK:

* Loops: "+ New loop" -> the form opens on the web's default cadence
  (Maintenance); the Self-paced / Fixed interval segments switch the fields
  (Fixed adds the interval, prefilled 5m); Create (Maintenance, empty prompt)
  -> `loop/create {prompt: "", mode: "maintenance"}` -> the server's
  maintenance loop row.
* Monitors: "+ New monitor" -> name / command (JSON array) / filter regex;
  a regex the server cannot compile -> the server's `monitor_invalid_spec`
  refusal is the dialog's alert line (role=alert in the web); the corrected
  create succeeds, the alert clears, the row appears.

Against `replay_serve --scenario a10` (r1 recorded lists + the faithful
`a10-autonomy-faithful.jsonl` replies). usage: a10_autonomy.py <desktop|phone> <outdir>
"""
import sys
import time

from a10_lib import Walk, checks_line, dialog_checks, run_session

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast).
WALK = {
    "name": "a10_autonomy",
    "title": "the Loops and Monitors create forms by CLICK (cadence segments, the server's refusal line)",
    "modes": ["desktop", "phone"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {
        100: {"checks": ["loops: + New loop CLICK", "loops: Fixed interval segment CLICK",
                         "loops: Self-paced segment CLICK", "loops: Create with an empty self-paced prompt",
                         "loops: Maintenance segment CLICK, then Create", "wire: loop/create x1"],
              "partial": "the maintenance creation sends one typed loop/create; self-paced and fixed are switched "
                         "and refused-when-empty, not each created"},
        102: {"checks": ["monitors: + New monitor CLICK", "monitors: Create CLICK -> monitor/create refused",
                         "monitors: the corrected create succeeds", "wire: monitor/create x2"],
              "partial": "create through a typed refusal then a success; pause / resume / delete are not walked here"},
    },
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/autonomy/{MODE}"


def numeric(W: Walk, name: str, prefix: str):
    W.wait(lambda: bool(W.visible("dialog_frame")), 6)
    c = dialog_checks(W.snap(), "dialog_frame", prefix, viewport="dialog_scroll")
    W.check(f"{name}: dialog numeric checks", c["ok"], checks_line(c))


def click_logged(W: Walk, wid: str, needle: str, expect=None, secs: float = 8.0) -> bool:
    time.sleep(0.4)
    W.mark()
    ok = W.click_in(wid, "dialog_scroll") if W.visible("dialog_scroll") else W.click(wid)
    logged = W.logged(needle, secs / 2) if ok else False
    if ok and not logged:
        W.note(f"RETRY {wid}")
        ok = W.click(wid)
        logged = W.logged(needle, secs / 2) if ok else False
    seen = W.wait(expect, secs) if (ok and expect) else True
    return ok and logged and seen


def type_into(W: Walk, wid: str, text: str) -> None:
    W.scroll_into(wid, "dialog_scroll")
    r = W.rect(wid)
    if not r:
        W.note(f"no field {wid}")
        return
    W.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    W.clear_field(30)
    W.type_text(text)
    W.dismiss_keyboard("dlg_" + wid.split("_")[1] + "_cf_title")


def walk(W: Walk) -> None:
    W.note("== 1. /loop -> + New loop: the form on the default cadence (Maintenance)")
    W.check("loops: /loop palette CLICK opens the dialog",
            W.palette_run("loo", "/loop") and W.wait_shown("dlg_loops_loops_card", 10))
    W.check("loops: + New loop CLICK -> the cadence form, Maintenance selected",
            click_logged(W, "dlg_loops_new_loop_control", "dialog form opened: loop.create",
                         lambda: bool(W.visible("dlg_loops_fm_seg_maintenance_control")))
            and not W.visible("dlg_loops_lf_interval"))
    numeric(W, "loop form (maintenance)", ("dlg_loops_",))
    W.shot(f"01-loop-form-maintenance-{MODE}")
    W.check("loops: Fixed interval segment CLICK -> the interval field appears prefilled 5m",
            click_logged(W, "dlg_loops_fm_seg_fixed_control", "dialog form mode: fixed",
                         lambda: W.text("dlg_loops_lf_interval") == "5m"))
    numeric(W, "loop form (fixed)", ("dlg_loops_",))
    W.shot(f"02-loop-form-fixed-{MODE}")
    W.check("loops: Self-paced segment CLICK -> no interval field",
            click_logged(W, "dlg_loops_fm_seg_self_paced_control", "dialog form mode: self_paced",
                         lambda: not W.visible("dlg_loops_lf_interval")))
    W.check("loops: Create with an empty self-paced prompt is refused ON the form (nothing sent)",
            click_logged(W, "dlg_loops_ff_submit_control", "dialog form refused",
                         lambda: "A prompt is required" in W.text("dlg_loops_ff_error"))
            and W.replay_saw("loop/create", 0) == 0)
    W.shot(f"03-loop-form-refused-{MODE}")
    W.check("loops: Maintenance segment CLICK, then Create -> loop/create maintenance",
            click_logged(W, "dlg_loops_fm_seg_maintenance_control", "dialog form mode: maintenance")
            and click_logged(W, "dlg_loops_ff_submit_control", "dialog form submitted: loop.create",
                             lambda: W.has_text("run maintenance checks"), 10))
    W.check("wire: loop/create x1 (maintenance)", W.replay_saw("loop/create", 4) == 1)
    numeric(W, "loops after create", ("dlg_loops_",))
    W.shot(f"04-loops-created-{MODE}")
    W.click("dialog_close")
    W.wait_shown("dialog_frame", 5, gone=True)

    W.note("== 2. /monitor -> + New monitor: a refused create is the alert line")
    W.check("monitors: /monitor palette CLICK opens the dialog",
            W.palette_run("moni", "/monitor") and W.wait_shown("dlg_monitors_t_title", 10))
    W.check("monitors: + New monitor CLICK -> the create form",
            click_logged(W, "dlg_monitors_new_monitor_control", "dialog form opened: monitor.create",
                         lambda: bool(W.visible("dlg_monitors_mf_name"))))
    type_into(W, "dlg_monitors_mf_name", "watch-build")
    type_into(W, "dlg_monitors_mf_argv", '["./scripts/watch.sh", "--verbose"]')
    type_into(W, "dlg_monitors_mf_filter", "(")
    numeric(W, "monitor form", ("dlg_monitors_",))
    W.shot(f"05-monitor-form-{MODE}")
    W.check("monitors: Create CLICK -> monitor/create refused by the server -> the dialog's alert line",
            click_logged(W, "dlg_monitors_ff_submit_control", "dialog form submitted: monitor.create",
                         lambda: "filter_regex does not compile" in W.text("dlg_monitors_dialog_notice"), 10))
    numeric(W, "monitors alert", ("dlg_monitors_",))
    W.shot(f"06-monitor-alert-{MODE}")
    W.check("monitors: + New monitor again, a valid regex, Create -> created, the alert clears",
            click_logged(W, "dlg_monitors_new_monitor_control", "dialog form opened: monitor.create",
                         lambda: bool(W.visible("dlg_monitors_mf_name"))))
    type_into(W, "dlg_monitors_mf_name", "watch-build")
    type_into(W, "dlg_monitors_mf_argv", '["./scripts/watch.sh", "--verbose"]')
    type_into(W, "dlg_monitors_mf_filter", "ERROR.*")
    W.check("monitors: the corrected create succeeds; the row appears and no alert remains",
            click_logged(W, "dlg_monitors_ff_submit_control", "dialog form submitted: monitor.create",
                         lambda: W.has_text("./scripts/watch.sh") and not W.visible("dlg_monitors_dialog_notice"), 10))
    W.check("wire: monitor/create x2 (refused, then created)", W.replay_saw("monitor/create", 4) == 2)
    numeric(W, "monitors after create", ("dlg_monitors_",))
    W.shot(f"07-monitor-created-{MODE}")
    W.click("dialog_close")
    W.wait_shown("dialog_frame", 5, gone=True)

    W.note("== 3. close suspends, reopen refreshes: /loop again re-reads the list")
    before = W.replay_saw("loop/list", 0)
    W.check("loops: reopen by palette CLICK -> a fresh loop/list read, the rows shown",
            W.palette_run("loo", "/loop") and W.wait_shown("dlg_loops_loops_card", 10)
            and W.wait(lambda: W.replay_saw("loop/list", 0) > before, 6),
            f"loop/list {before} -> {W.replay_saw('loop/list', 0)}")
    W.click("dialog_close")


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
