#!/usr/bin/env python3
"""Sidebar footer routing by native clicks, desktop and phone.

Theme opens the unified Preferences selector, closing the phone drawer first.
Its label/icon follow the selected choice; Save writes the same display file.
Fleet, Settings, and collapsed-rail targets keep their existing placement.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a26_lib as L  # noqa: E402

WALK = {
    "name": "a26_footer",
    "title": "Sidebar footer: Fleet, the unified Theme shortcut, Settings — the web's order; icons only in the rail",
    "modes": ["desktop", "phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "history"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}", "OCTOS_PROFILE_ID": "dsflash",
                    "OCTOS_BEARER": "replay", "OCTOSCODE_THEME": "light"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}", "{mode}", "{fixture_log}", "{out}"]}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 900,
    "rows": {},
}

PREF = os.environ.get("OCTOSCODE_DISPLAY_PREFS_PATH", "")
ROWS = ["sb_add_hit", "fleet_nav_hit", "sb_theme_hit", "sb_settings_hit"]
# A Mac's sidebar has no theme row: Settings > Preferences holds the choice.
MAC_ROWS = ["sb_add_hit", "history_nav_hit", "fleet_nav_hit", "sb_settings_hit"]


def stored_theme():
    try:
        return json.load(open(PREF)).get("theme")
    except (OSError, ValueError):
        return None


def footer_checks(w, tag):
    sn = w.snap()
    col = w.rect("threads_column", sn=sn)
    mac = w.mode != "phone"
    rects = [w.rect(r, sn=sn) for r in (MAC_ROWS if mac else ROWS)]
    w.check(f"{tag}: the footer rows are on screen (+ Add workspace, Fleet, theme, Settings)"
            if not mac else f"{tag}: the footer rows are on screen (Add Workspace, Session History, Fleet, Settings)",
            all(rects), f"{rects}")
    if not all(rects):
        return sn
    ys = [r[1] for r in rects]
    w.check(f"{tag}: in the web's order, one stack (Fleet, then the theme toggle, then Settings)"
            if not mac else f"{tag}: one stack (Add Workspace, Session History, Fleet, Settings)",
            ys == sorted(ys) and all(abs((ys[i + 1] - ys[i]) - rects[i][3]) <= 1 for i in range(3)), f"y={ys}")
    w.check(f"{tag}: one row height and one left edge (A3's row metrics)",
            len({r[3] for r in rects}) == 1 and len({r[0] for r in rects}) == 1 and rects[0][3] >= 28, f"{rects}")
    w.check(f"{tag}: inside the sidebar column", bool(col) and all(L.inside(r, col) for r in rects), f"col={col}")
    labels = {i: w.text(i, sn=sn) for i in ("fleet_nav_label", "sb_theme_label", "sb_settings_label")}
    if mac:
        w.check(f"{tag}: the labels (Fleet, Settings), and no theme row",
                labels["fleet_nav_label"] == "Fleet" and labels["sb_settings_label"] == "Settings"
                and not w.visible("sb_theme_hit", sn) and not w.visible("sb_theme_label", sn), f"{labels}")
    else:
        w.check(f"{tag}: the labels (Fleet, the theme, Settings)",
                labels["fleet_nav_label"] == "Fleet" and labels["sb_settings_label"] == "Settings"
                and labels["sb_theme_label"] in ("System", "Light", "Dark"), f"{labels}")
    for lid, hid in ((("sb_settings_label", "sb_settings_hit"),) if mac else (("sb_theme_label", "sb_theme_hit"), ("sb_settings_label", "sb_settings_hit"))):
        lr, hr = w.rect(lid, sn=sn), w.rect(hid, sn=sn)
        w.check(f"{tag}: {lid} sits whole inside its row", bool(lr and hr and L.inside(lr, hr)), f"{lr} in {hr}")
    return sn


def theme_icon(w, sn=None):
    sn = sn if sn is not None else w.snap()
    on = [i for i in ("sb_theme_ic_system", "sb_theme_ic_light", "sb_theme_ic_dark") if w.visible(i, sn)]
    return on


def choose_theme(w, label, look=None):
    L.open_drawer(w)
    if w.mode == "phone":
        w.click("sb_theme_hit")
        w.check("Theme opens Preferences", w.wait(lambda: w.text("set_title") == "Preferences", 6))
        w.check("Theme closes the phone drawer", not w.visible("drawer_scrim"))
    else:
        w.check("Settings > Preferences opens from the footer", L.open_settings_from_footer(w) and L.open_preferences(w))
    w.click("pal_" + label.lower())
    def selected():
        sn = w.snap()
        row = w.rect("pal_" + label.lower(), sn=sn)
        return bool(row and any(x.get("i") == "rd_on" and w.shown(x) and L.inside(x["r"], row) for x in sn))
    w.check(f"one selector chooses {label}", w.wait(selected, 6))
    # A Mac keeps a preference when it changes; a phone has a Save button.
    if w.mode == "phone":
        w.click("prefs_save")
    if PREF:
        # On a Mac System is what's kept already until another choice is made.
        w.check(f"{label}: the shared preference stores the choice",
                stored_theme() == label.lower() or (w.mode != "phone" and label == "System" and stored_theme() is None),
                repr(stored_theme()))
    L.close_settings(w)
    if look:
        ok, detail = L.draws_look(w, look)
        w.check(f"{label}: the whole app re-themes live", ok, detail)
    if w.mode != "phone":
        return
    L.open_drawer(w)
    want_icon = {"System": "sb_theme_ic_system", "Light": "sb_theme_ic_light", "Dark": "sb_theme_ic_dark"}[label]
    w.check(f"{label}: the shortcut shows its icon", theme_icon(w) == [want_icon])


def main():
    w = L.walk_from_argv()
    mode = w.mode
    w.mark()
    if mode == "phone":
        w.check("the drawer opens from the header's menu", L.open_drawer(w))
    sn = footer_checks(w, "light")
    # A desktop's OCTOSCODE_THEME seed wins over the saved System (screens/theme.rs).
    first = "System" if mode == "phone" else "Light"
    if mode == "phone":
        w.check(f"the shortcut reads the initial {first} preference", w.text("sb_theme_label", sn=sn) == first and theme_icon(w, sn) == [f"sb_theme_ic_{first.lower()}"])
    w.shot(f"{mode}-footer-light")
    choose_theme(w, "System")
    choose_theme(w, "Dark", "dark")
    footer_checks(w, "dark")
    w.shot(f"{mode}-footer-dark")
    choose_theme(w, "Light", "light")
    # Settings from the footer.
    w.mark()
    w.click("sb_settings_hit")
    opened = w.wait(lambda: bool(w.visible("set_title")), 6)
    w.check("Settings opens from the footer's Settings", opened, repr(w.text("set_title")))
    if mode == "phone":
        w.check("…closing the drawer first", w.wait(lambda: not w.visible("drawer_scrim"), 4))
    w.shot(f"{mode}-footer-settings")
    w.check("Settings closes", L.close_settings(w))
    # Fleet from the footer (A4's destination).
    if mode == "phone":
        L.open_drawer(w)
    w.mark()
    w.click("fleet_nav_hit")
    w.check("Fleet opens from the footer's Fleet", w.logged("sidebar: fleet", 6))
    time.sleep(1.0)
    w.shot(f"{mode}-footer-fleet")
    w.key("Escape")
    time.sleep(0.8)
    if mode == "desktop":
        if w.visible("b3_close") or w.visible("board3_dock"):
            w.click("b3_close")
            time.sleep(0.6)
        # A Mac's sidebar collapses away with its footer; the toolbar keeps
        # Settings and a button that brings the sidebar back.
        w.mark()
        w.click("sidebar_collapse")
        w.check("collapsed: the sidebar and its footer are gone, and the toolbar can bring them back",
                w.wait(lambda: bool(w.visible("hd_expand_hit")), 6)
                and not w.visible("sb_settings_hit") and not w.visible("fleet_nav_hit") and not w.visible("history_nav_hit"))
        time.sleep(0.6)
        w.shot(f"{mode}-collapsed-light")
        w.click("settings_open_hit")
        w.check("collapsed: the toolbar's Settings opens", w.wait(lambda: bool(w.visible("set_title")), 6))
        L.close_settings(w)
        w.click("hd_expand_hit")
        w.check("the column expands back with its labels", w.wait(lambda: bool(w.visible("sb_settings_label")), 6))
    sys.exit(w.summary())


if __name__ == "__main__":
    main()
