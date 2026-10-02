#!/usr/bin/env python3
"""A29 — parity row 6: the /btw aside owned by the Session that asked, by
CLICKS and keys in the real app, desktop and phone (360x780).

Against replay_serve's `btw` scenario (live-gate's handshake: `session/btw`
advertised as a method; three Sessions of the `octos` workspace; each aside
answered 10 s after it is asked with the asking Session's id echoed; the 2nd
aside's socket drops, the 3rd aside fails):

 1. a main prompt starts a turn in X; `/btw <question>` typed in X's
    composer + Return while it works: the panel shows above X's composer,
    Answering… (board 4 region 4) — the wire's session/btw carries X's id;
 2. once the main turn settles, a CLICK on Y's sidebar row while X's aside
    still answers: Y shows no panel, X's row carries the aside marker (blue
    dot), Y's none;
 3. X's answer arrives while Y is on screen: it never shows in Y; X's marker
    turns to answered;
 4. a CLICK back on X: the whole Markdown answer (region 5a);
 5. the chevron folds it to one row (5b) and unfolds it;
 6. a new aside whose connection drops: the stale copy, red lead + muted
    cause (5c), still stale once the link is back; an empty composer Return
    dismisses it;
 7. `/btw` with no question: the usage hint, nothing sent;
 8. a failed aside: the web's FAILED copy; Close;
 9. Close while answering: gone, and its late answer stays hidden;
10. an ordinary prompt admitted in X clears the settled aside;
11. Chinese: Settings > Preferences > 简体中文, a new aside: 旁问 — /btw,
    关闭, 正在回答…, 此旁问不会保存到对话。 (Z5).

Every panel state is measured: the panel spans the composer's width and sits
above it, its labels lie inside it with no overlaps, its controls are >= 28
px, its height <= min(50% of the window, 480).
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bridgeauth  # noqa: E402,F401  (D10c: the bridge's per-launch token on every request)
import a26_lib as L  # noqa: E402
from a10_lib import checks_line, dialog_checks, inside, overlap  # noqa: E402

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
WALK = {
    "name": "a29_btw_aside",
    "title": "The /btw aside owned by the Session that asked: answering above its composer, a switch away and back, the sidebar marker, answered, folded, Close, stale, the usage hint, cleared by a prompt, zh",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "btw", "--adopt-turn-ids",
                         "--delay-ms", "40", "--btw-delay-ms", "10000", "--btw-drop", "2", "--btw-fail", "3"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "dsflash",
                    "OCTOS_BEARER": "replay", "OCTOSCODE_THEME": "light"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}", "{mode}", "{fixture_log}", "{out}"]}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    # docs/walk-rows.csv row 95: e2e/native-workflows.spec.ts:240 "native
    # asides remain ephemeral and owned by their original Session across
    # delayed A to B replies".
    "rows": {95: ["wire: session/btw #1 carries X", "Y: no aside", "X's row marks its answering aside",
                  "the answer never shows in Y", "back on X: the whole answer", "Close: the aside is gone",
                  "the late answer stays hidden"]},
}

X = "Fix steer queue drop on reconnect"
Y = "Why is hydrate slow?"
XID, YID = "dsflash:main", "dsflash:hydrate"
Q1 = "why does redeliver drain the whole queue first?"
Q2 = "does redeliver keep the order across a reconnect?"
Q3 = "is the queue persisted across restarts?"
Q4 = "what does steer_dropped count?"
Q5 = "which test covers redeliver?"
QZH = "为什么要先排空队列？"
MAIN = "In one short paragraph: what does main.rs print, and why?"


# ------------------------------------------------------------------ helpers
def replay_lines(w):
    if not w.replay_log or not w.replay_log.exists():
        return []
    return w.replay_log.read_text().splitlines()


def asked(w, n):
    """The replay's line for the n-th session/btw (its session_id and question)."""
    return next((l for l in replay_lines(w) if f"session/btw #{n} asked:" in l), "")


def answered(w, n):
    return next((l for l in replay_lines(w) if f"session/btw #{n} answered" in l), "")


def focus_composer(w):
    comp = w.composer()
    if comp is None:
        return False
    x, y, ww, h = comp["r"]
    w.click_xy(x + ww / 2, y + h / 2)
    return True


def send(w, text):
    """CLICK the composer, type, Return — the production send (per key for the
    command's leading slash, as a person types it)."""
    if not focus_composer(w):
        return False
    w.clear_field()
    w.type_text(text)
    time.sleep(0.3)
    w.key("Return")
    time.sleep(0.4)
    return True


def sidebar_open(w):
    if w.mode == "phone" and not w.visible("sb_new_chat_hit"):
        w.dismiss_keyboard()
        w.click("sidebar_toggle_hit")
        w.wait(lambda: bool(w.visible("sb_new_chat_hit")), 5)


def sidebar_close(w):
    if w.mode == "phone" and w.visible("sb_new_chat_hit"):
        w.click("drawer_close")
        w.wait(lambda: not w.visible("sb_new_chat_hit"), 5)
        time.sleep(0.4)


def row_title(w, title, sn=None):
    sn = sn if sn is not None else w.snap()
    for s in sn:
        t = (s.get("t") or "").strip()
        if s.get("i") != "sb_r_title" or not w.shown(s):
            continue
        if t == title or (t.endswith("…") and title.startswith(t[:-1].rstrip())):
            return s
    return None


def open_row(w, title):
    sidebar_open(w)
    r = row_title(w, title)
    if r is None:
        w.note(f"CLICK row {title!r} — not visible")
        return False
    x, y, ww, h = r["r"]
    w.note(f"CLICK row {title!r} r={r['r']}")
    w.click_xy(x + ww / 2, y + h / 2)
    time.sleep(1.2)
    return True


def mark(w, title, sn=None):
    """(marker rect, blue dot shown, red dot shown) on the row titled `title`."""
    sn = sn if sn is not None else w.snap()
    row = row_title(w, title, sn)
    if row is None:
        return None, False, False
    cy = row["r"][1] + row["r"][3] / 2

    def on_row(wid):
        return [s["r"] for s in sn if s.get("i") == wid and w.shown(s) and abs(s["r"][1] + s["r"][3] / 2 - cy) < 14]

    chip = on_row("sb_r_aside")
    return (chip[0] if chip else None), bool(on_row("sb_r_aside_run")), bool(on_row("sb_r_aside_fail"))


def mark_layout(w, title):
    """The marker sits on its row between the title and the time, inside the
    sidebar, overlapping neither label."""
    sn = w.snap()
    chip, _, _ = mark(w, title, sn)
    row = row_title(w, title, sn)
    col = w.rect("threads_column", sn=sn)
    if not (chip and row and col):
        return w.check(f"layout: {title[:14]}…'s marker is on its row", False, f"chip={chip} row={row and row['r']}")
    cy = row["r"][1] + row["r"][3] / 2
    times = [s["r"] for s in sn if s.get("i") == "sb_r_time" and w.shown(s) and abs(s["r"][1] + s["r"][3] / 2 - cy) < 14]
    ok = (inside(chip, col) and not overlap(chip, row["r"]) and not any(overlap(chip, t) for t in times)
          and abs(chip[1] + chip[3] / 2 - cy) <= 3 and chip[0] >= row["r"][0])
    return w.check(f"layout: {title[:14]}…'s marker sits on its row (centred, after the title, before the time)", ok,
                   f"chip={chip} title={row['r']} time={times[:1]}")


def panel_shown(w, sn=None):
    return bool(w.visible("btw_aside", sn))


def panel_checks(w, name):
    """The panel's numeric checks: the composer's width, above the composer,
    the height bound, labels inside / not overlapping, controls >= 28."""
    sn = w.snap()
    mod = w.module_rect(sn)
    c = dialog_checks(sn, "btw_aside", ("btw_aside",), module=mod, viewport="btw_aside_body")
    fr = c.get("frame")
    comp = w.rect("i0_composer", sn=sn)
    cap = min(0.5 * mod[3], 480.0) if mod else 480.0
    aligned = bool(fr and comp) and abs(fr[0] - comp[0]) <= 1.5 and abs((fr[0] + fr[2]) - (comp[0] + comp[2])) <= 1.5
    above = bool(fr and comp) and fr[1] + fr[3] <= comp[1] + 0.5
    bounded = bool(fr) and fr[3] <= cap + 1.0
    wide = bool(fr) and (w.mode == "phone" or fr[2] >= 600)
    ok = bool(c.get("ok")) and aligned and above and bounded and wide
    w.check(f"{name}: panel {checks_line(c)} composer-width={aligned} above={above} h<={cap:.0f}={bounded}"
            + (f" desktop-width>=600={wide}" if w.mode == "desktop" else ""),
            ok, json.dumps({"frame": fr, "composer": comp, "c": c})[:400])
    return ok


def capture(w, name):
    w.dismiss_keyboard()
    time.sleep(0.5)
    return w.shot(f"{w.mode}-{name}")


# --------------------------------------------------------------------- walk
def working(w):
    return any((s.get("i") or "").endswith("_workingrow_label") for s in w.snap() if w.shown(s))


def main():
    w = L.walk_from_argv()
    w.mark()
    w.check("the app opened X (the composer is ready)", w.wait(lambda: w.composer() is not None, 20))
    sidebar_open(w)
    w.check("the sidebar lists X and Y", w.wait(lambda: bool(row_title(w, X)) and bool(row_title(w, Y)), 15))
    sidebar_close(w)

    # 1. A main turn works in X; the aside is asked in X meanwhile (region 4).
    send(w, MAIN)
    w.check("the main prompt started a turn", w.wait(lambda: w.replay_saw("turn/start") >= 1, 8))
    w.dismiss_keyboard()
    send(w, f"/btw {Q1}")
    w.check("wire: session/btw #1 carries X's id and the question",
            w.wait(lambda: f"session_id={XID}" in asked(w, 1) and Q1 in asked(w, 1), 8), asked(w, 1))
    w.check("wire: session/btw #1 carries X", f"session_id={XID}" in asked(w, 1))
    w.check("X: the panel shows above X's composer", w.wait(lambda: panel_shown(w), 6))
    w.check("X: Aside — /btw · Answering… · the scope line · the note",
            w.wait(lambda: w.text("btw_aside_title") == "Aside — /btw" and w.text("btw_aside_status_label") == "Answering…", 4)
            and w.text("btw_aside_scope") == f"octos · {X}"
            and w.text("btw_aside_note") == "This aside is not saved to the conversation.",
            f"{w.text('btw_aside_title')!r} {w.text('btw_aside_status_label')!r} {w.text('btw_aside_scope')!r}")
    w.check("X: the accepted aside consumed the composer's text",
            w.wait(lambda: (w.composer() or {}).get("val", "") == "", 3))
    w.check("X: the main turn keeps working above the aside", working(w))
    panel_checks(w, "answering")
    capture(w, "1-answering")

    # 2. The main turn settles; then a switch to Y while X's aside answers.
    w.check("the main turn settles", w.wait(lambda: not working(w), 20))
    w.check("CLICK Y's row while X's aside answers", not answered(w, 1) and open_row(w, Y))
    w.check("Y is on screen (the header names Y)", w.wait(lambda: w.text("hd_title") == Y, 8), repr(w.text("hd_title")))
    sidebar_close(w)
    w.check("Y: no aside (Y holds none)", w.wait(lambda: not panel_shown(w), 6))
    sidebar_open(w)
    chip, run, _ = mark(w, X)
    w.check("X's row marks its answering aside (blue dot)", bool(chip) and run, f"chip={chip} run={run}")
    ychip, _, _ = mark(w, Y)
    w.check("Y's row carries no marker", ychip is None, f"{ychip}")
    mark_layout(w, X)
    capture(w, "2-switched-marker")
    sidebar_close(w)

    # 3. X's answer arrives while Y is on screen.
    w.check("wire: session/btw #1 answered (X's id echoed)", w.wait(lambda: bool(answered(w, 1)), 15), answered(w, 1))
    time.sleep(1.0)
    w.check("the answer never shows in Y", not panel_shown(w) and not w.has_text("Draining first keeps the order stable"))
    sidebar_open(w)
    chip, run, _ = mark(w, X)
    w.check("X's row marks an answered aside (no dot)", bool(chip) and not run, f"chip={chip} run={run}")
    mark_layout(w, X)
    capture(w, "3-answered-in-x-marker")

    # 4. Back on X: the whole answer (region 5a).
    w.check("CLICK X's row", open_row(w, X))
    sidebar_close(w)
    w.check("back on X: the whole answer",
            w.wait(lambda: "Draining first keeps the order stable" in w.text("btw_aside_answer"), 8),
            repr(w.text("btw_aside_answer"))[:120])
    w.check("back on X: no Answering… once answered", not w.visible("btw_aside_status_label"))
    sidebar_open(w)
    chip, _, _ = mark(w, X)
    w.check("X on screen: its row shows no marker (the panel shows instead)", chip is None, f"{chip}")
    sidebar_close(w)
    panel_checks(w, "answered")
    capture(w, "4-answered")

    # 5. Fold and unfold (region 5b).
    w.check("CLICK the chevron", w.click("btw_aside_toggle_hit"))
    if w.mode == "phone":
        # A 336 px card: "Answered" yields its room to the question.
        w.check("folded: one row (› Aside — /btw · question · Close; the question keeps >= 60 px)",
                w.wait(lambda: not w.visible("btw_aside_body") and not w.visible("btw_aside_state")
                       and (w.rect("btw_aside_question") or [0, 0, 0, 0])[2] >= 60, 4),
                f"question={w.rect('btw_aside_question')}")
    else:
        w.check("folded: one row (› Aside — /btw · question · Answered · Close)",
                w.wait(lambda: not w.visible("btw_aside_body") and w.text("btw_aside_state") == "Answered", 4),
                repr(w.text("btw_aside_state")))
    fr = w.rect("btw_aside")
    w.check("folded: the row is one line tall (<= 52 px)", bool(fr) and fr[3] <= 52, f"{fr}")
    panel_checks(w, "folded")
    capture(w, "5-folded")
    w.check("CLICK the chevron again: unfolded", w.click("btw_aside_toggle_hit")
            and w.wait(lambda: bool(w.visible("btw_aside_body")), 4))

    # 6. The connection drops while an aside answers: stale (region 5c).
    send(w, f"/btw {Q2}")
    w.check("wire: session/btw #2 carries X", w.wait(lambda: f"session_id={XID}" in asked(w, 2), 8), asked(w, 2))
    w.check("the connection drops while it answers: the stale copy",
            w.wait(lambda: w.text("btw_aside_error") == "The Session connection changed before the aside completed.", 10),
            repr(w.text("btw_aside_error")))
    w.check("stale: the muted cause", w.text("btw_aside_error_detail") == "Ask again when it is ready.")
    w.check("the link comes back", w.wait(lambda: not w.visible("link_row") and w.composer() is not None, 40))
    time.sleep(1.0)
    w.check("still stale once the link is back (terminal)",
            w.text("btw_aside_error") == "The Session connection changed before the aside completed.")
    panel_checks(w, "stale")
    capture(w, "6-stale")
    # An empty composer Return dismisses it (App.tsx:1170-1173).
    focus_composer(w)
    w.clear_field()
    w.key("Return")
    w.check("an empty submit dismisses the aside", w.wait(lambda: not panel_shown(w), 4))
    w.dismiss_keyboard()

    # 7. /btw with no question: the usage hint, nothing sent.
    before = sum(1 for line in replay_lines(w) if "asked:" in line)
    send(w, "/btw")
    w.check("the usage hint (intent.ts:103)",
            w.wait(lambda: w.has_text("/btw is unavailable — Use /btw <question> for a temporary side answer. "
                                      "Nothing was sent to the model."), 6))
    time.sleep(0.5)
    after = sum(1 for line in replay_lines(w) if "asked:" in line)
    w.check("no question: nothing sent", after == before, f"{before} -> {after}")
    w.check("no panel for a hint", not panel_shown(w))
    capture(w, "7-hint")

    # 8. A failed aside: the web's FAILED copy; Close.
    send(w, f"/btw {Q3}")
    w.check("wire: session/btw #3 carries X", w.wait(lambda: f"session_id={XID}" in asked(w, 3), 8), asked(w, 3))
    w.check("failed: the web's copy (red lead, muted cause)",
            w.wait(lambda: w.text("btw_aside_error") == "The aside could not be answered.", 15)
            and w.text("btw_aside_error_detail") == "Try again.", repr(w.text("btw_aside_error")))
    panel_checks(w, "failed")
    capture(w, "8-failed")
    w.check("CLICK Close on the failed aside", w.click("btw_aside_close_hit") and w.wait(lambda: not panel_shown(w), 4))

    # 9. Close while answering; the late answer stays hidden.
    send(w, f"/btw {Q4}")
    w.check("wire: session/btw #4 carries X", w.wait(lambda: f"session_id={XID}" in asked(w, 4), 8), asked(w, 4))
    w.check("answering again", w.wait(lambda: w.text("btw_aside_status_label") == "Answering…", 4))
    w.dismiss_keyboard()
    w.check("CLICK Close", w.click("btw_aside_close_hit"))
    w.check("Close: the aside is gone", w.wait(lambda: not panel_shown(w), 4))
    w.check("wire: session/btw #4 answered later", w.wait(lambda: bool(answered(w, 4)), 15))
    time.sleep(1.0)
    w.check("the late answer stays hidden", not panel_shown(w))
    sidebar_open(w)
    chip, _, _ = mark(w, X)
    w.check("…and no marker", chip is None)
    sidebar_close(w)

    # 10. An ordinary prompt admitted in X clears the settled aside.
    send(w, f"/btw {Q5}")
    w.check("wire: session/btw #5 carries X", w.wait(lambda: f"session_id={XID}" in asked(w, 5), 8), asked(w, 5))
    w.check("answered", w.wait(lambda: bool(w.text("btw_aside_answer")), 15))
    w.mark()
    send(w, "And add a test for it.")
    w.check("an admitted prompt clears the settled aside", w.wait(lambda: not panel_shown(w), 6))
    w.check("log: the aside cleared on admission", w.logged("cleared (a prompt was admitted)", 4))
    w.check("the prompt went out as a turn", w.wait(lambda: w.replay_saw("turn/start") >= 2, 8))
    w.wait(lambda: not working(w), 15)

    # 11. Chinese (Z5).
    w.check("Settings opens from the sidebar footer", L.open_settings_from_footer(w))
    w.check("Preferences shows", L.open_preferences(w))
    r = w.rect("lang_zh")
    if r:
        w.click_xy(r[0] + r[2] / 2, r[1] + r[3] / 2)
    w.check("Language: 简体中文", w.logged("a24 language -> zh", 6))
    L.close_settings(w)
    time.sleep(0.6)
    send(w, f"/btw {QZH}")
    w.check("zh: 旁问 — /btw · 关闭 · 正在回答… · 此旁问不会保存到对话。",
            w.wait(lambda: w.text("btw_aside_title") == "旁问 — /btw" and w.text("btw_aside_status_label") == "正在回答…", 8)
            and w.text("btw_aside_close_label") == "关闭" and w.text("btw_aside_note") == "此旁问不会保存到对话。",
            f"{w.text('btw_aside_title')!r} {w.text('btw_aside_close_label')!r} {w.text('btw_aside_status_label')!r} {w.text('btw_aside_note')!r}")
    panel_checks(w, "zh answering")
    capture(w, "9-zh-answering")
    w.check("zh: answered", w.wait(lambda: bool(w.text("btw_aside_answer")), 15))
    panel_checks(w, "zh answered")
    capture(w, "10-zh-answered")
    w.check("zh: CLICK the chevron", w.click("btw_aside_toggle_hit"))
    if w.mode == "phone":
        w.check("zh folded: one row, the question keeps its room",
                w.wait(lambda: not w.visible("btw_aside_body") and not w.visible("btw_aside_state"), 4))
    else:
        w.check("zh folded: 旁问 — /btw · question · 已回答 · 关闭",
                w.wait(lambda: not w.visible("btw_aside_body") and w.text("btw_aside_state") == "已回答", 4),
                repr(w.text("btw_aside_state")))
    panel_checks(w, "zh folded")
    capture(w, "11-zh-folded")
    sys.exit(w.summary())


if __name__ == "__main__":
    main()
