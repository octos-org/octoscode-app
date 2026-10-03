#!/usr/bin/env python3
"""A3 — the board-2 chrome click walk: every control is reached from the real
UI by a CLICK at its laid-out rect (the LESSONS rule), and each step asserts
the app's own effect (a /snap visibility or text, and/or the routed log line).
It also runs the numeric /snap layout checks (no truncated header tabs,
uniform row heights, hit targets >= 28 px, no overflow past the window).

The app must already run hidden with the instrument on PORT and the board-2
fixture seed, e.g.

  OCTOSCODE_BOARD2_SEED=1 OCTOSCODE_DESIGN_DIR=$PWD/design \\
  MAKEPAD_WM_TEST_APP=octoscode HEADLESS_ARGS="--module octoscode" \\
    bash harness/headless.sh start <host-bin> 8413
  python3 tools/walk/a3_chrome_walk.py 8413 desktop
  bash harness/headless.sh stop 8413

`phone` mode expects the shell's phone style and the 360x780 frame:
  ... HEADLESS_ARGS="--module octoscode --test-action page:0" \\
      OCTOSENSE_WINDOW_SIZE=360x780 ... ; then tap the OctosCode icon (the
  walk does it) and run `python3 tools/walk/a3_chrome_walk.py 8413 phone`.

The Stop dialog is only ever CANCELLED here (never confirm against a shared
server); the confirmed path is covered at the wire by tests/a3_chrome.rs.
Exit status: 0 when every step passes.
"""
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import bridgeauth  # noqa: E402,F401  (D10c: the bridge token on every request)

# A11: the walk aggregator's convention (tools/walk/native.py; never imported).
WALK = {
    "name": "a3_chrome",
    "title": "board 2 chrome: sidebar (groups, search, sort, menu, rail, drawer) and Settings",
    "modes": ["desktop", "phone"],
    "app": {"env": {"OCTOSCODE_BOARD2_SEED": "1"},
            "ready": ["sidebar_toggle_hit", "sb_new_chat_hit", "conversation_column"]},
    "runs": [{"argv": ["{port}", "{mode}"]}],
    "timeout": 600,
    "rows": {
        4: {"checks": ["Desktop notifications toggles"],
            "partial": "the opt-in only; OS notices staying silent while reading are not observable headless"},
        144: ["All shows the flat list", "By workspace regroups", "layout: session rows are 32 px",
              "layout: group headers are 36 px", "A group header collapses it", "…and expands it again"],
        163: {"checks": ["Permissions preset Full access is offered"],
              "partial": "the full-access confirmation is a8_session's"},
        185: {"checks": ["Search highlights the match", "Search names the empty group",
                         "Clear search restores every row"],
              "partial": "'replies use the chat column' is not asserted"},
        191: {"checks": ["Stop server… opens the confirm", "Cancel closes the confirm",
                         "Cancel sent no server/shutdown"],
              "partial": "the confirmed shutdown and its unconfirmed-result copy are covered at the wire "
                         "(tests/a3_chrome.rs), never confirmed against a server"},
        227: {"checks": {"phone": ["Menu opens the drawer", "drawer width is min(320, w - 48)",
                                   "Opening a row dismisses the drawer"]},
              "partial": "the navigation drawer only; the composer staying on screen is not asserted here"},
        231: {"checks": ["Sort cycles the order", "A session row opens its session"],
              "partial": "'tracks which Session was last opened' is not asserted"},
    },
}

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8413
MODE = sys.argv[2] if len(sys.argv) > 2 else "desktop"
BASE = f"http://127.0.0.1:{PORT}"
RESULTS = []


def get(path, timeout=20):
    # A11: an input route with wait=1 answers HTTP 404 when its frame was
    # coalesced — the input itself was delivered (tools/walk/walk_env.py does
    # the same). Raising there crashed the walk midway (12 checks never ran).
    try:
        with urllib.request.urlopen(BASE + path, timeout=timeout) as r:
            return r.read().decode()
    except urllib.error.HTTPError:
        if path.startswith(("/click", "/t?", "/k?", "/m?")):
            return ""
        raise


def snap():
    return json.loads(get("/snap?all=1"))["s"]


def visible(widgets, wid):
    return [w for w in widgets if w.get("i") == wid and w.get("v", 1) != 0 and w["r"][2] > 0 and w["r"][3] > 0]


def is_shown(wid):
    return bool(visible(snap(), wid))


def rect(wid, nth=0):
    hits = sorted(visible(snap(), wid), key=lambda w: (w["r"][1], w["r"][0]))
    return hits[nth]["r"] if len(hits) > nth else None


def click(wid, nth=0):
    r = rect(wid, nth)
    if r is None:
        return False
    x, y, w, h = r
    get(f"/click?x={x + w / 2}&y={y + h / 2}&wait=1")
    time.sleep(0.15)
    return True


def type_text(text):
    get("/t?" + urllib.parse.urlencode({"t": text, "wait": 1}))
    time.sleep(0.15)


LOG_SEQ = [0]


def log_since():
    d = json.loads(get(f"/log?since={LOG_SEQ[0]}"))
    LOG_SEQ[0] = d.get("n", LOG_SEQ[0])
    return d.get("l", [])


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))


def inside(child, parent):
    """Whether a visible `child` lies inside the visible `parent` rect."""
    sn = snap()
    for c in visible(sn, child):
        for p in visible(sn, parent):
            cx, cy, cw, ch = c["r"]
            px, py, pw, ph = p["r"]
            if cx >= px and cy >= py and cx + cw <= px + pw and cy + ch <= py + ph:
                return True
    return False


def texts(wid):
    return [w.get("t", "") for w in visible(snap(), wid)]


def step(name, wid, expect, nth=0, log_needle=None):
    log_since()
    clicked = click(wid, nth)
    lines = log_since()
    ok = clicked and expect()
    if log_needle:
        ok = ok and any(log_needle in l for l in lines)
    check(name, ok, f"clicked={clicked}" + (f" log~{log_needle!r}" if log_needle else ""))


def layout_checks():
    s = snap()
    # 1. No truncated header tabs: each label sits fully inside its pill.
    for label, pill in (("hd_review_label", "hd_review"), ("hd_settings_label", "hd_settings")):
        l, p = visible(s, label), visible(s, pill)
        if not l:
            check(f"layout: {label} is icon-only in this mode", MODE == "phone")
            continue
        lx, ly, lw, lh = l[0]["r"]
        px, py, pw, ph = p[0]["r"]
        check(f"layout: {label} inside its pill (not truncated)",
              lx >= px and lx + lw <= px + pw and lw >= 40,
              f"label {l[0]['r']} pill {p[0]['r']}")



def row_checks():
    s = snap()
    # 2. Uniform row heights: every session row 32, every group header 36.
    rows = [w for w in visible(s, "sb_r_open")]
    groups = [w for w in visible(s, "sb_g_hit")]
    check("layout: session rows are 32 px", rows and all(w["r"][3] == 32 for w in rows),
          f"{sorted({w['r'][3] for w in rows})}")
    check("layout: group headers are 36 px", groups and all(w["r"][3] == 36 for w in groups),
          f"{sorted({w['r'][3] for w in groups})}")
    # 3. Consistent left inset: every row and header starts at one x.
    xs = {w["r"][0] for w in rows + groups}
    check("layout: rows and headers share one left edge", len(xs) == 1, f"{xs}")


def hit_checks():
    s = snap()
    # 4. Hit targets >= 28 px for the visible chrome controls.
    small = []
    for wid in ("sb_new_chat_hit", "sb_search_clear", "sg_hit", "sb_sort", "sb_g_more",
                "sb_g_hit", "sb_r_open", "sb_add_hit", "review_open_hit", "settings_open_hit",
                "sidebar_toggle_hit", "sidebar_collapse", "drawer_close"):
        for w in visible(s, wid):
            if w["r"][2] < 28 or w["r"][3] < 28:
                small.append((wid, w["r"]))
    check("layout: chrome hit targets >= 28 px", not small, f"{small}")
    # 5. Nothing overflows the module's frame on the right.
    root = visible(s, "base") or visible(s, "first_run")
    if root:
        right = root[0]["r"][0] + root[0]["r"][2]
        over = [(w["i"], w["r"]) for w in s
                if w.get("v", 1) != 0 and w["r"][2] > 0 and w["i"] in
                ("hd_settings", "hd_review", "sb_search", "thread_list", "sidebar_dock")
                and w["r"][0] + w["r"][2] > right + 0.5]
        check("layout: chrome stays inside the frame", not over, f"{over}")


def settings_walk():
    step("Settings opens from the header", "settings_open_hit",
         lambda: is_shown("settings_drawer"), log_needle="settings.panel.open")
    # A9 added Preferences before About.
    sections = ["general", "permissions", "model", "capabilities", "sandbox", "connection", "preferences", "about"]
    cell = "rl_hit" if MODE == "phone" else "nv_hit"
    for i, sec in enumerate(sections):
        step(f"Settings nav -> {sec}", cell, lambda sec=sec: is_shown(f"sec_{sec}"),
             nth=i, log_needle=f"settings.section.{sec}")
    click(cell, 0)  # back to General
    s = snap()
    small = [(w["i"], w["r"]) for wid in ("tg_hit", cell, "settings_close", "set_back", "server_stop_request")
             for w in visible(s, wid) if w["r"][2] < 28 or w["r"][3] < 28]
    check("layout: settings hit targets >= 28 px", not small, f"{small}")
    # A25: the row follows the OS (crate::attention). A hidden test app is a
    # bare binary, which macOS never lets post a notice: the row then reads
    # "Unavailable" with no toggle to press (the web's disabled button,
    # GeneralSettingsContent.tsx:236). With a notification backend (an .app,
    # the phone, OCTOSCODE_NOTIFY_FAKE) the click asks the OS and the toggle
    # flips once it answers.
    if not is_shown("tg_notify"):
        state = texts("notify_state")
        check("Desktop notifications toggles", state[:1] == ["Unavailable"]
              and any("unavailable" in t for t in texts("notify_help")),
              f"this process has no notification backend: the row reads {state} (A25)")
    else:
        before = inside("tg_on", "tg_notify")

        def flipped():
            end = time.time() + 4
            while time.time() < end:
                if inside("tg_on", "tg_notify") != before:
                    return True
                time.sleep(0.3)
            return False

        step("Desktop notifications toggles", "tg_notify", flipped, log_needle="notifications_toggle.toggle")
        click("tg_notify")  # restore
    step("Stop server… opens the confirm", "server_stop_request",
         lambda: is_shown("stop_dialog"), log_needle="server.stop.request")
    log_since()
    step("Cancel closes the confirm", "server_stop_cancel",
         lambda: not is_shown("stop_dialog"), log_needle="server.stop.cancel")
    lines = log_since()
    check("Cancel sent no server/shutdown", not any("settings sent: server.stop.confirm" in l for l in lines))
    click(cell, 1)
    step("Permissions preset Full access is offered", "perm_full",
         lambda: is_shown("sec_permissions"), log_needle="perm_full.select")
    click(cell, 2)
    step("Thinking Off selects", "th_off",
         lambda: inside("sg_pill", "th_off"), log_needle="settings.thinking.off")
    step("Thinking On selects", "th_on",
         lambda: inside("sg_pill", "th_on"), log_needle="settings.thinking.on")
    click(cell, sections.index("sandbox"))  # A36: Capabilities sits before Sandbox
    before = inside("tg_on", "tg_sb_network")
    step("Sandbox: Network access toggles", "tg_sb_network",
         lambda: inside("tg_on", "tg_sb_network") != before, log_needle="sandbox_network.toggle")
    click("tg_sb_network")  # restore
    close = "set_back" if MODE == "phone" else "settings_close"
    step("Settings closes", close, lambda: not is_shown("settings_drawer"),
         log_needle="settings.panel.close")


def sidebar_walk():
    if MODE == "phone":
        step("Menu opens the drawer", "sidebar_toggle_hit",
             lambda: is_shown("drawer_scrim") and is_shown("threads_column"),
             log_needle="drawer.open")
        r = rect("threads_column")
        check("drawer width is min(320, w - 48)", r is not None and abs(r[2] - 312) <= 1, f"{r}")
    row_checks()
    hit_checks()
    step("All shows the flat list", "sg_hit", lambda: not is_shown("sb_g_hit"), nth=1,
         log_needle="sidebar.mode.flat")
    step("By workspace regroups", "sg_hit", lambda: is_shown("sb_g_hit"), nth=0,
         log_needle="sidebar.mode.grouped")
    click("sb_search")
    type_text("hydrate")
    hits = texts("sb_r_hit")
    check("Search highlights the match", "hydrate" in hits, f"{hits}")
    check("Search names the empty group", any("match" in t for t in texts("sb_n_text")))
    step("Clear search restores every row", "sb_search_clear",
         lambda: len(visible(snap(), "sb_r_open")) >= 5, log_needle="search.clear")
    step("A group header collapses it", "sb_g_hit", lambda: is_shown("sb_g_count"), nth=0,
         log_needle="workspace.toggle")
    step("…and expands it again", "sb_g_hit", lambda: not is_shown("sb_g_count"), nth=0,
         log_needle="workspace.toggle")
    step("The ⋯ opens the workspace menu", "sb_g_more", lambda: is_shown("sb_menu_items"), nth=1,
         log_needle="workspace.menu")
    step("Rename opens the form", "sb_menu_rename", lambda: is_shown("sb_rename_form"),
         log_needle="workspace.rename")
    step("Cancel closes the form", "sb_rename_cancel", lambda: not is_shown("sb_menu_dock"),
         log_needle="workspace.rename.cancel")
    sort0 = texts("sb_sort_label")
    step("Sort cycles the order", "sb_sort", lambda: texts("sb_sort_label") != sort0,
         log_needle="sidebar.sort")
    click("sb_sort")
    click("sb_sort")  # back to Recent
    step("A session row opens its session", "sb_r_open", lambda: True, nth=1,
         log_needle="session.open")
    if MODE == "phone":
        check("Opening a row dismisses the drawer", not is_shown("drawer_scrim"))
    else:
        step("The sidebar collapses to its rail", "sidebar_collapse",
             lambda: is_shown("oc_sidebar_rail") and (rect("threads_column") or [0, 0, 0])[2] == 56,
             log_needle="sidebar.collapse")
        step("The rail's search expands the sidebar", "rb_hit",
             lambda: is_shown("sb_search"), nth=2, log_needle="search.open")
        # A2: + Add workspace opens board 1's workspace surfaces (the folder
        # browser over the picker; the picker alone when browsing is not
        # advertised), each with its own back chevron.
        def soon(pred):
            for _ in range(20):
                if pred():
                    return True
                time.sleep(0.25)
            return False
        step("Add workspace opens the workspace picker/browser", "sb_add_hit",
             lambda: soon(lambda: is_shown("b1_card")), log_needle="workspace.add")
        if is_shown("b1_br_back"):
            step("The browser's back returns to the picker", "b1_br_back",
                 lambda: soon(lambda: is_shown("b1_pk_back")), log_needle="browser.close")
        step("The picker's back closes it", "b1_pk_back",
             lambda: soon(lambda: not is_shown("b1_card")), log_needle="picker.close")
        # A10: the header's Review entry opens the web's DiffReviewDialog
        # (a board-3 modal with its own 28 px close), not the old sheet.
        step("Review opens from the header", "review_open_hit",
             lambda: soon(lambda: is_shown("b3_diff_eyebrow")), log_needle="ToggleReview")
        click("b3_close")


def main():
    if MODE == "phone" and not is_shown("conversation_column"):
        # Open the app from the phone home (the OctosCode icon).
        get("/click?x=153&y=363&wait=1")
        time.sleep(3)
    log_since()
    layout_checks()
    if MODE != "phone":
        row_checks()
        hit_checks()
    sidebar_walk()
    if MODE == "phone" and is_shown("drawer_scrim"):
        click("drawer_close")
    settings_walk()
    failed = [n for n, ok, _ in RESULTS if not ok]
    print(f"== WALK {MODE}: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
