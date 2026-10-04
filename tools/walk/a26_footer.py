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


def stored_theme():
    try:
        return json.load(open(PREF)).get("theme")
    except (OSError, ValueError):
        return None


def footer_checks(w, tag):
    sn = w.snap()
    col = w.rect("threads_column", sn=sn)
    rects = [w.rect(r, sn=sn) for r in ROWS]
    w.check(f"{tag}: the footer rows are on screen (+ Add workspace, Fleet, theme, Settings)", all(rects), f"{rects}")
    if not all(rects):
        return sn
    ys = [r[1] for r in rects]
    w.check(f"{tag}: in the web's order, one stack (Fleet, then the theme toggle, then Settings)",
            ys == sorted(ys) and all(abs((ys[i + 1] - ys[i]) - rects[i][3]) <= 1 for i in range(3)), f"y={ys}")
    w.check(f"{tag}: one row height and one left edge (A3's row metrics)",
            len({r[3] for r in rects}) == 1 and len({r[0] for r in rects}) == 1 and rects[0][3] >= 28, f"{rects}")
    w.check(f"{tag}: inside the sidebar column", bool(col) and all(L.inside(r, col) for r in rects), f"col={col}")
    labels = {i: w.text(i, sn=sn) for i in ("fleet_nav_label", "sb_theme_label", "sb_settings_label")}
    w.check(f"{tag}: the labels (Fleet, the theme, Settings)",
            labels["fleet_nav_label"] == "Fleet" and labels["sb_settings_label"] == "Settings"
            and labels["sb_theme_label"] in ("System", "Light", "Dark"), f"{labels}")
    for lid, hid in (("sb_theme_label", "sb_theme_hit"), ("sb_settings_label", "sb_settings_hit")):
        lr, hr = w.rect(lid, sn=sn), w.rect(hid, sn=sn)
        w.check(f"{tag}: {lid} sits whole inside its row", bool(lr and hr and L.inside(lr, hr)), f"{lr} in {hr}")
    return sn


def theme_icon(w, sn=None):
    sn = sn if sn is not None else w.snap()
    on = [i for i in ("sb_theme_ic_system", "sb_theme_ic_light", "sb_theme_ic_dark") if w.visible(i, sn)]
    return on


def choose_theme(w, label, look=None):
    L.open_drawer(w)
    w.click("sb_theme_hit")
    w.check("Theme opens Preferences", w.wait(lambda: w.text("set_title") == "Preferences", 6))
    if w.mode == "phone":
        w.check("Theme closes the phone drawer", not w.visible("drawer_scrim"))
    w.click("pal_" + label.lower())
    def selected():
        sn = w.snap()
        row = w.rect("pal_" + label.lower(), sn=sn)
        return bool(row and any(x.get("i") == "rd_on" and w.shown(x) and L.inside(x["r"], row) for x in sn))
    w.check(f"one selector chooses {label}", w.wait(selected, 6))
    w.click("prefs_save")
    if PREF:
        w.check(f"{label}: the shared preference stores the choice", stored_theme() == label.lower())
    L.close_settings(w)
    if look:
        ok, detail = L.draws_look(w, look)
        w.check(f"{label}: the whole app re-themes live", ok, detail)
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
    w.check("the shortcut reads the initial System preference", w.text("sb_theme_label", sn=sn) == "System" and theme_icon(w, sn) == ["sb_theme_ic_system"])
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
        # The collapsed rail: the icons only, centred, and still working.
        w.mark()
        w.click("sidebar_collapse")
        w.wait(lambda: bool(w.visible("rail_expand")), 6)
        time.sleep(0.6)
        sn = w.snap()
        hidden = [i for i in ("fleet_nav_label", "sb_theme_label", "sb_settings_label") if w.visible(i, sn)]
        w.check("rail: the footer labels are gone (icons only)", not hidden, f"still shown: {hidden}")
        hits = [w.rect(h, sn=sn) for h in ("fleet_nav_hit", "sb_theme_hit", "sb_settings_hit")]
        col = w.rect("threads_column", sn=sn)
        w.check("rail: the three footer targets stay inside the 56 px rail, >= 28 px",
                all(hits) and col and col[2] <= 57 and all(L.inside(h, col) and h[3] >= 28 and h[2] >= 28 for h in hits),
                f"col={col} hits={hits}")
        svgs = [x["r"] for x in sn if x.get("ty") == "Svg" and w.shown(x) and col and L.inside(x["r"], col)]
        rail_icon = w.rect("rb_icon", sn=sn)
        centre = (rail_icon[0] + rail_icon[2] / 2) if rail_icon else None
        foot_icons = [r for r in svgs if hits[0] and r[1] >= hits[0][1] - 1]
        offs = [round((r[0] + r[2] / 2) - centre, 1) for r in foot_icons] if centre else []
        w.check("rail: the footer icons sit on the rail buttons' centre line",
                len(foot_icons) == 3 and all(abs(o) <= 1.5 for o in offs), f"centre={centre} offsets={offs}")
        w.shot(f"{mode}-rail-light")
        choose_theme(w, "Dark", "dark")
        w.shot(f"{mode}-rail-dark")
        w.click("sb_settings_hit")
        w.check("rail: Settings opens", w.wait(lambda: bool(w.visible("set_title")), 6))
        L.close_settings(w)
        choose_theme(w, "Light", "light")
        w.click("rail_expand")
        w.check("the column expands back with its labels", w.wait(lambda: bool(w.visible("sb_theme_label")), 6))
    sys.exit(w.summary())


if __name__ == "__main__":
    main()
