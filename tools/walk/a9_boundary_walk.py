#!/usr/bin/env python3
"""A9 — the error-boundary walk. The app runs with a panic armed by the test
seam OCTOSCODE_PANIC_PROBE (inert in production); everything AFTER the
panic is the production path, driven by CLICKS:

  A9_SCENARIO=fatal   (OCTOSCODE_PANIC_PROBE=fatal:settings.panel.open)
      click Settings -> the crash screen alone; Copy diagnostics -> Copied;
      Report this crash (hidden window: logged, not opened); Reload app ->
      the app starts again and is usable.
  A9_SCENARIO=modal   (OCTOSCODE_PANIC_PROBE=surface:activity)
      /activity from the palette -> "Activity unavailable" (modal); the
      conversation stays; Close -> gone; Settings still opens.
  A9_SCENARIO=inline  (OCTOSCODE_PANIC_PROBE=surface:fleet)
      the sidebar's Fleet -> "Fleet unavailable" in the conversation column
      (no backdrop); the sidebar still works; Close -> gone.

  A9_SCENARIO=fatal OCTOSCODE_PANIC_PROBE=fatal:settings.panel.open \\
    a9_walk.sh a9_boundary_walk.py desktop <shots>
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a9_activity_walk as w  # noqa: E402

check, soon, snap, rect, is_shown, text_of, click, click_rect, log_since, shot = (
    w.check, w.soon, w.snap, w.rect, w.is_shown, w.text_of, w.click, w.click_rect, w.log_since, w.shot,
)
MODE = w.MODE
SCENARIO = os.environ.get("A9_SCENARIO", "fatal")


def connected():
    return soon(lambda: is_shown("i0_composer_0") and not is_shown("connect_card"), tries=60)


def fatal():
    log_since()
    click("settings_open_hit")
    lines = log_since()
    check("the panic is caught by the fatal boundary", soon(lambda: is_shown("a9_cr_card")) and any("a9 fatal boundary" in l for l in lines + log_since()))
    s = snap()
    check("the crash screen is drawn alone (no chrome behind it)",
          not is_shown("threads_column", s) and not is_shown("conversation_column", s) and not is_shown("settings_drawer", s))
    check("the web's copy", text_of("a9_cr_title", s) == "Client view unavailable")
    report = text_of("a9_cr_report", s) or ""
    check("the report is redacted (token + bearer)", "token=[redacted]" in report and "Bearer [redacted]" in report
          and "probe-secret-123" not in report and "probe-bearer-456" not in report, report[:120])
    card = rect("a9_cr_card", s=s)
    small = [(i, rect(i, s=s)) for i in ("a9_cr_reload", "a9_cr_copy", "a9_cr_report_link") if not rect(i, s=s) or rect(i, s=s)[3] < 28]
    check("crash actions are >= 28 px targets", not small, f"{small}")
    check("the card fits the module", card is not None, f"{card}")
    shot(f"{MODE}-crash")
    click("a9_cr_copy")
    check("Copy diagnostics -> Copied", soon(lambda: text_of("a9_cr_copy_label") == "Copied") and any("diagnostics copied" in l for l in log_since()))
    log_since()
    click("a9_cr_report_link")
    check("Report this crash (hidden window: logged, not opened)", any("report link https://github.com/octos-org/octoscode-app/issues/new" in l for l in log_since()))
    shot(f"{MODE}-crash-copied")
    log_since()
    click("a9_cr_reload")
    check("Reload app starts the app again", soon(lambda: not is_shown("a9_cr_card")) and connected())
    check("…and it is usable (the composer is back)", is_shown("i0_composer_0"))


def modal():
    log_since()
    ok = w.open_activity()
    lines = log_since()
    check("the Activity surface fails under its own boundary",
          soon(lambda: is_shown("a9_un_panel")) and any("a9 surface boundary: Activity unavailable" in l for l in lines + log_since()), f"{ok}")
    s = snap()
    check("'Activity unavailable' with the web's copy", text_of("a9_un_title", s) == "Activity unavailable"
          and text_of("a9_un_p0", s) == "This view could not be displayed. Other parts of the app remain available.")
    check("a modal (over a backdrop)", is_shown("a9_un_mask", s))
    # (On a phone the sidebar is a closed drawer; the conversation is there.)
    check("the session owner stays mounted (the conversation and sidebar are there)",
          is_shown("conversation_column", s) and (MODE == "phone" or is_shown("threads_column", s)))
    small = [(i, rect(i, s=s)) for i in ("a9_un_close", "a9_un_reload") if not rect(i, s=s) or rect(i, s=s)[3] < 28]
    check("its actions are >= 28 px targets", not small, f"{small}")
    shot(f"{MODE}-unavailable-modal")
    click("a9_un_close")
    check("Close dismisses it", soon(lambda: not is_shown("a9_un_panel")))
    check("the rest of the app still works (Settings opens)", click("settings_open_hit") and soon(lambda: is_shown("settings_drawer")))
    click("set_back" if MODE == "phone" else "settings_close")


def inline():
    log_since()
    if MODE == "phone":
        # The Fleet entry lives in the phone's navigation drawer.
        click("sidebar_toggle_hit")
        soon(lambda: is_shown("fleet_nav_hit"))
    click("fleet_nav_hit")
    check("the Fleet pane fails under its own boundary",
          soon(lambda: is_shown("a9_un_panel")) and any("a9 surface boundary: Fleet unavailable" in l for l in log_since()))
    s = snap()
    check("an inline section (no backdrop)", not is_shown("a9_un_mask", s) and text_of("a9_un_title", s) == "Fleet unavailable")
    col, panel, head = rect("conversation_column", s=s), rect("a9_un_panel", s=s), rect("oc_header", s=s)
    check("…placed in the conversation column", col and panel and panel[0] >= col[0] and panel[0] + panel[2] <= col[0] + col[2] + 0.5, f"{panel} in {col}")
    check("…under its header (Review / Settings stay reachable)", head and panel and panel[1] >= head[1] + head[3], f"{panel} under {head}")
    shot(f"{MODE}-unavailable-inline")
    log_since()
    if MODE == "phone":
        click("sidebar_toggle_hit")
        soon(lambda: is_shown("sb_new_chat_hit"))
    click("sb_new_chat_hit")
    check("the sidebar still works beside it (New chat routes)", any("new_chat" in l or "new chat" in l for l in log_since() + log_since()))
    if MODE == "phone" and is_shown("drawer_scrim"):
        click("drawer_close")
    click("a9_un_close")
    check("Close dismisses it", soon(lambda: not is_shown("a9_un_panel")))


def main():
    if MODE == "phone" and not is_shown("conversation_column"):
        w.get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    check("connected", connected())
    {"fatal": fatal, "modal": modal, "inline": inline}[SCENARIO]()
    failed = [n for n, ok, _ in w.RESULTS if not ok]
    print(f"== WALK a9 boundary {SCENARIO} {MODE}: {len(w.RESULTS) - len(failed)}/{len(w.RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
