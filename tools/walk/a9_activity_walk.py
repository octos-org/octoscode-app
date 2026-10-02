#!/usr/bin/env python3
"""A9 — the Activity click walk: every Activity control is reached from the
real UI by a CLICK at its laid-out rect, and each step asserts the app's own
effect (a /snap visibility or text, and/or the routed log line, and the
replay server's wire log). It also runs the numeric /snap layout checks (the
dialog inside the window, hit targets >= 28 px, row texts inside their rows,
one left edge for every row, no text under the action pill).

The app must run hidden against the replay server's `activity` scenario
(recorded c24b task snapshots; see examples/replay_serve.rs):

  target/debug/examples/replay_serve 8429 --scenario activity > tmp/replay-8429.log &
  # Brief §8 (test isolation): the app's stores live in a fresh directory,
  # never the operator's ~/.octoscode (the other A9 walks launch the same way).
  S=$(mktemp -d)
  export OCTOSCODE_DRAFTS_FILE=$S/drafts.json OCTOSCODE_CREDENTIALS_DIR=$S/cred \\
    OCTOSCODE_PREF_PATH=$S/display.json OCTOSCODE_NOTIFICATIONS_FILE=$S/notify.json \\
    OCTOSCODE_RECENTS_DIR=$S/recents OCTOSCODE_SHOW_THINKING_FILE=$S/thinking.json \\
    OCTOSCODE_DOWNLOAD_DIR=$S/downloads OCTOSCODE_DISPLAY_PREFS_PATH=$S/display-v1.json
  OCTOS_BASE_URL=http://127.0.0.1:8429 OCTOS_PROFILE_ID=a9walk \\
  OCTOSCODE_DESIGN_DIR=$PWD/design MAKEPAD_WM_TEST_APP=octoscode \\
  HEADLESS_ARGS="--module octoscode" bash harness/headless.sh start <host-bin> 8419
  python3 tools/walk/a9_activity_walk.py 8419 desktop tmp/replay-8429.log [shots-dir]
  bash harness/headless.sh stop 8419

`phone` mode expects the shell's phone style and the 360x780 frame
(HEADLESS_ARGS="--module octoscode --test-action page:0",
OCTOSENSE_WINDOW_SIZE=360x780); the walk opens the app from the phone home.
Exit status: 0 when every step passes.
"""
import json
import os
import sys
import time
import urllib.parse
import urllib.request

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8419
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
REPLAY_LOG = sys.argv[3] if len(sys.argv) > 3 else None
SHOTS = sys.argv[4] if len(sys.argv) > 4 else None
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []


def get(path, timeout=20):
    """An instrument call. The instrument can answer 404 while a frame is in
    flight: a READ (/snap, /g, /log) is retried; an ACTION (/t, /click, /k)
    may already have happened, so it is never repeated."""
    action = path.startswith(("/t?", "/click", "/k?"))
    for attempt in range(6):
        try:
            with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
                return r.read()
        except urllib.error.HTTPError:
            if action:
                time.sleep(0.4)
                return b""
            if attempt == 5:
                raise
            time.sleep(0.4)


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(widgets, wid):
    return [w for w in widgets if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def is_shown(wid, s=None):
    return bool(visible(s or snap(), wid))


def rect(wid, nth=0, s=None):
    hits = sorted(visible(s or snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[nth]["r"] if len(hits) > nth else None


def text_of(wid, s=None):
    hits = visible(s or snap(), wid)
    return hits[0].get("t", "") if hits else None


def click_rect(r):
    x, y, w, h = r
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.25)


def click(wid, nth=0):
    r = rect(wid, nth)
    if r is None:
        return False
    click_rect(r)
    return True


def type_text(text):
    get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
    time.sleep(0.25)


def key(code):
    get(f"/k?c={code}")
    time.sleep(0.25)


LOG_SEQ = [0]


def log_since():
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return d.get("l", [])


def wire():
    if not REPLAY_LOG or not os.path.exists(REPLAY_LOG):
        return []
    with open(REPLAY_LOG) as f:
        return f.read().splitlines()


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))


def soon(pred, tries=24, pause=0.25):
    for _ in range(tries):
        if pred():
            return True
        time.sleep(pause)
    return False


def shot(name):
    if not SHOTS:
        return
    os.makedirs(SHOTS, exist_ok=True)
    path = os.path.join(SHOTS, f"{name}.png")
    png = None
    for _ in range(5):
        try:
            png = get("/g?raw=1", timeout=30)
            break
        except Exception:  # the capture endpoint can 404 between frames
            time.sleep(0.5)
    if png is None:
        print(f"SHOT {name}: capture unavailable")
        return
    with open(path, "wb") as f:
        f.write(png)
    os.system(f"sips -Z 1400 '{path}' >/dev/null 2>&1")
    with open(os.path.join(SHOTS, f"{name}.snap.json"), "w") as f:
        json.dump([w for w in snap() if w.get("i", "").startswith("a9_")], f)
    print(f"SHOT {path}")


def rows(s=None):
    s = s or snap()
    out = []
    for w in s:
        i = w.get("i", "")
        if i.startswith("a9_act_row_") and i.count("_") == 3 and w.get("v", 1) != 0 and w["r"][2] > 0:
            out.append((int(i.rsplit("_", 1)[1]), w["r"]))
    return sorted(out)


def composer_text(s=None):
    """The composer's value (`val`; `t` on older instruments)."""
    v = visible(s or snap(), "i0_composer_0")
    return (v[0].get("val") or v[0].get("t") or "") if v else ""


def clear_composer():
    """Empty the composer before typing: it may hold an interrupted turn's
    restored prompt (A6, like the web) or a saved draft (A7). End + one
    Backspace per character - the instrument's synthetic Cmd+A does not
    reach the TextInput's select-all. Waited presses (`wait=1`) and one
    settle at the end, so a step racing a held reply (the switch window)
    is not slowed down."""
    n = len(composer_text())
    get("/k?c=end&wait=1")
    for _ in range(max(n + 1, 8)):
        get("/k?c=backspace&wait=1")
    time.sleep(0.35)


def open_activity(wait_dialog=True):
    """Open the palette from the composer and click the /activity row."""
    comp = rect("i0_composer_0")
    if comp is None:
        return False
    click_rect(comp)
    # Clear whatever draft is there, then the palette's "/" + the filter.
    clear_composer()
    type_text("/")
    type_text("act")
    if not soon(lambda: "/activity" in [w.get("t") for w in visible(snap(), "palette_row_name")]):
        return False
    s = snap()
    names = visible(s, "palette_row_name")
    hits = visible(s, "palette_row_hit")
    for n in names:
        if n.get("t") == "/activity":
            row = [h for h in hits if abs(h["r"][1] - n["r"][1]) < 20]
            click_rect((row[0] if row else n)["r"])
            if not wait_dialog:
                return True
            return soon(lambda: is_shown("a9_act_dialog"))
    return False


def layout_checks(tag):
    s = snap()
    root = visible(s, "OctoscodeView") or visible(s, "base")
    dlg = rect("a9_act_dialog", s=s)
    check(f"{tag}: the dialog is shown", dlg is not None, f"{dlg}")
    if dlg is None:
        return
    if root:
        rx, ry, rw, rh = root[0]["r"]
        inside = dlg[0] >= rx - 0.5 and dlg[1] >= ry - 0.5 and dlg[0] + dlg[2] <= rx + rw + 0.5 and dlg[1] + dlg[3] <= ry + rh + 0.5
        check(f"{tag}: the dialog lies inside the module", inside, f"dialog {dlg} module {root[0]['r']}")
        if MODE == "phone":
            check(f"{tag}: full-bleed on the phone (the web's 100% x 100%)", abs(dlg[2] - rw) <= 1, f"{dlg[2]} vs {rw}")
        else:
            check(f"{tag}: desktop width is min(900, w - 32)", abs(dlg[2] - min(900, rw - 32)) <= 1, f"{dlg[2]}")
    small = []
    for wid in ("a9_act_close", "a9_act_search_field"):
        for w in visible(s, wid):
            if w["r"][3] < 28 or w["r"][2] < 28:
                small.append((wid, w["r"]))
    for w in s:
        i = w.get("i", "")
        if w.get("v", 1) == 0 or w["r"][2] == 0:
            continue
        if (i.startswith("a9_act_filter_") and i.endswith("_box")) or (i.startswith("a9_act_row_") and i.endswith("_act_box")):
            if w["r"][3] < 28:
                small.append((i, w["r"]))
    check(f"{tag}: hit targets >= 28 px", not small, f"{small}")
    rs = rows(s)
    xs = {r[0] for _, r in rs}
    check(f"{tag}: rows share one left edge", len(xs) <= 1, f"{xs}")
    # Every row text stays inside its row (no truncation past the edge) and,
    # on a desktop row, left of the action pill.
    bad = []
    for n, r in rs:
        rx, ry, rw, rh = r
        pill = rect(f"a9_act_row_{n}_act_box", s=s)
        for part in ("title", "session", "detail"):
            t = rect(f"a9_act_row_{n}_{part}", s=s)
            if t is None:
                bad.append((n, part, "missing"))
                continue
            if t[0] < rx - 0.5 or t[0] + t[2] > rx + rw + 0.5 or t[1] < ry - 0.5 or t[1] + t[3] > ry + rh + 0.5:
                bad.append((n, part, t, r))
            if MODE != "phone" and pill and t[0] + t[2] > pill[0] - 4:
                bad.append((n, part, "under the pill", t, pill))
    check(f"{tag}: row texts inside their rows, clear of the pill", not bad, f"{bad[:3]}")
    over = []
    for w in s:
        i = w.get("i", "")
        if i.startswith("a9_act_") and w.get("v", 1) != 0 and w["r"][2] > 0:
            x, y, ww, hh = w["r"]
            if x + ww > dlg[0] + dlg[2] + 0.5 or y + hh > dlg[1] + dlg[3] + 0.5:
                if i not in ("a9_act_root", "a9_act_mask", "a9_act_backdrop", "a9_act_backdrop_box") and not i.startswith("a9_act_row_"):
                    over.append((i, w["r"]))
    check(f"{tag}: nothing spills past the card", not over, f"{over[:3]}")


def main():
    if MODE == "phone" and not is_shown("conversation_column"):
        get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    soon(lambda: is_shown("i0_composer_0"), tries=40)
    log_since()
    wire0 = len(wire())

    # 1. /activity from the palette, by click.
    ok = open_activity()
    lines = log_since()
    check("palette /activity opens Activity", ok and any("a9 action activity.open" in l for l in lines))
    check("the catalog reads arrive (3 rows)", soon(lambda: len(rows()) == 3, tries=40), f"{len(rows())} rows")
    asked = [l for l in wire()[wire0:] if "-> task/list" in l]
    check("one task/list per confirmed session, nothing opened",
          len(asked) == 5 and not any("<- session/open" in l for l in wire()[wire0:]), f"{asked}")
    s = snap()
    labels = [w.get("t") for w in s if w.get("i", "").startswith("a9_act_filter_") and w.get("i", "").endswith("_label") and w.get("v", 1) != 0]
    check("status filters carry the counts", labels == ["All 3", "Running 1", "Failed 1", "Done 1"], f"{labels}")
    check("the failed reads fail closed into the status line",
          text_of("a9_act_error", s) == "2 Session task snapshots unavailable", f"{text_of('a9_act_error', s)!r}")
    check("footer names the refresh", text_of("a9_act_footer_status", s) == "Read-only · refreshes every 10 seconds")
    order = [text_of(f"a9_act_row_{n}_title", s) for n, _ in rows(s)]
    check("rows: running, failed, done", order == ["c24b-probe", "Rebuild after the octos-core bump", "c24b-probe completed"], f"{order}")
    acts = [text_of(f"a9_act_row_{n}_act_label", s) for n, _ in rows(s)]
    check("Inspect on the current session, Open session elsewhere", acts == ["Inspect", "Open session", "Open session"], f"{acts}")
    layout_checks("open")
    shot(f"{MODE}-activity")

    # 2. The search filters by visibility (no remount).
    log_since()
    click("a9_act_search")
    type_text("fork")
    vis = [n for n, _ in rows()]
    check("search 'fork' leaves the fork row", vis == [2], f"{vis}")
    check("typing never remounts the dialog", not any("a9 activity mounted" in l for l in log_since()))
    # 3. A filter tap (re-lowered) keeps the typed text.
    rb = rect("a9_act_filter_2_box")
    if rb:
        click_rect(rb)
    soon(lambda: is_shown("a9_act_empty"))
    check("Failed + 'fork' shows the named empty state",
          text_of("a9_act_empty_detail") == "No failed task matches “fork”.", f"{text_of('a9_act_empty_detail')!r}")
    shot(f"{MODE}-activity-empty")
    click_rect(rect("a9_act_filter_0_box"))
    click("a9_act_search")
    for _ in range(6):
        key("Backspace")
    check("All + cleared search shows every row", soon(lambda: len(rows()) == 3), f"{len(rows())}")

    # 4. Inspect (current session) opens the task view.
    log_since()
    click("a9_act_row_0_act")
    lines = log_since()
    check("Inspect opens the Tasks view and closes Activity",
          any("dialog.open.tasks" in l for l in lines)
          and soon(lambda: not is_shown("a9_act_dialog") and is_shown("dialog_frame")), "")
    # The Tasks view is A5's dialog: its own close button (a phone has no
    # hardware Escape).
    click("dialog_close")
    check("…which closes back to the conversation", soon(lambda: not is_shown("dialog_frame")))

    # 5. Open session (another session) switches through the sidebar path.
    open_activity()
    soon(lambda: len(rows()) == 3, tries=40)
    log_since()
    wire1 = len(wire())
    click("a9_act_row_1_act")
    lines = log_since()
    check("Open session closes Activity and opens the owning session",
          any("a9 activity: open session" in l and ":bump" in l for l in lines)
          and soon(lambda: any("<- session/open" in l for l in wire()[wire1:])), "")
    # 6. While that switch is in flight (the replay holds its reply back),
    # Activity warns and refuses another switch.
    open_activity()
    soon(lambda: len(rows()) >= 1, tries=40)
    s = snap()
    check("a switch in flight shows the warning",
          text_of("a9_act_warning_text", s) == "Finish the workspace transition before opening another session.",
          f"{text_of('a9_act_warning_text', s)!r}")
    pills = [n for n, _ in rows(s) if is_shown(f"a9_act_row_{n}_act", s)]
    check("…and its Open session pills route nothing", not any(text_of(f"a9_act_row_{n}_act_label", s) == "Open session" for n in pills), f"{pills}")
    layout_checks("blocked")
    shot(f"{MODE}-activity-blocked")
    # 7. Close by the × (and, on a desktop card, by the backdrop).
    log_since()
    click("a9_act_close")
    check("× closes Activity", soon(lambda: not is_shown("a9_act_dialog")))
    if MODE != "phone":
        soon(lambda: not any("holding" in l for l in wire()[-1:]), tries=30)
        time.sleep(6)
        open_activity()
        soon(lambda: len(rows()) >= 1, tries=40)
        dlg = rect("a9_act_dialog")
        if dlg:
            get(f"/click?x={dlg[0] - 8}&y={dlg[1] + dlg[3] / 2}&wait=1")
        check("the backdrop closes Activity", soon(lambda: not is_shown("a9_act_dialog")))
    if MODE != "phone":
        # A phone frame has no hardware Escape (the × above is its close).
        check("Activity reopens", open_activity())
        key("Escape")
        check("Escape closes Activity", soon(lambda: not is_shown("a9_act_dialog")))

    failed = [n for n, ok, _ in RESULTS if not ok]
    print(f"== WALK a9 activity {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
