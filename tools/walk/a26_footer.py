#!/usr/bin/env python3
"""A26 — the sidebar footer walk (parity row shell/g-settings "Sidebar footer
entries: Fleet navigation entry, theme toggle (System/Light/Dark), Settings
entry"), in the WEB's layout (ProductSidebar.tsx:969-1016), by CLICKS.

Desktop: + Add workspace, Fleet, the theme toggle and Settings stack in the
column's footer (one row height, one left edge); the toggle cycles Light ->
System -> Dark -> Light, re-theming the whole app at once (the sidebar's own
background is sampled from the /g capture) and saving the choice; Settings
opens Settings; Fleet opens the Fleet pane; the collapsed rail keeps the three
icons only, centred on the rail buttons' line, and they still work.
Phone: the same rows at the bottom of the drawer; the toggle keeps the drawer
open; Settings closes the drawer first and opens the Settings sheet.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a26_lib as L  # noqa: E402

WALK = {
    "name": "a26_footer",
    "title": "Sidebar footer: Fleet, the theme toggle (live re-theme, saved), Settings — the web's order; icons only in the rail",
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

PREF = os.environ.get("OCTOSCODE_PREF_PATH", "")
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


def cycle_to(w, label, look=None):
    """Click the theme toggle once; check the label, the icon and (for a
    light / dark result) the drawn look and the saved file."""
    w.mark()
    w.click("sb_theme_hit")
    ok = w.wait(lambda: w.text("sb_theme_label") == label, 6)
    w.check(f"the theme toggle cycles to {label}", ok and w.logged(f"theme -> {label.lower()}", 4), repr(w.text("sb_theme_label")))
    want_icon = {"System": "sb_theme_ic_system", "Light": "sb_theme_ic_light", "Dark": "sb_theme_ic_dark"}[label]
    w.check(f"{label}: the toggle shows its icon ({want_icon[12:]})", theme_icon(w) == [want_icon], f"{theme_icon(w)}")
    want_saved = {"System": None, "Light": "light", "Dark": "dark"}[label]
    if PREF:
        w.check(f"{label}: the choice is saved at once (system removes it)", stored_theme() == want_saved, f"{stored_theme()!r}")
    if look:
        time.sleep(0.6)
        ok, detail = L.draws_look(w, look)
        w.check(f"{label}: the whole app re-themes live", ok, detail)


def main():
    w = L.walk_from_argv()
    mode = w.mode
    w.mark()
    if mode == "phone":
        w.check("the drawer opens from the header's menu", L.open_drawer(w))
    sn = footer_checks(w, "light")
    w.check("the toggle reads the current theme (Light)", w.text("sb_theme_label", sn=sn) == "Light" and theme_icon(w, sn) == ["sb_theme_ic_light"])
    w.shot(f"{mode}-footer-light")
    cycle_to(w, "System")
    cycle_to(w, "Dark", "dark")
    if mode == "phone":
        w.check("the drawer stays open on the toggle", bool(w.visible("sb_settings_hit")))
    footer_checks(w, "dark")
    w.shot(f"{mode}-footer-dark")
    cycle_to(w, "Light", "light")
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
        w.mark()
        w.click("sb_theme_hit")
        w.check("rail: the theme icon still cycles (Light -> System)", w.logged("theme -> system", 6))
        w.click("sb_theme_hit")
        w.check("rail: …and again (System -> Dark)", w.logged("theme -> dark", 6))
        time.sleep(0.6)
        w.shot(f"{mode}-rail-dark")
        w.click("sb_settings_hit")
        w.check("rail: the Settings icon opens Settings", w.wait(lambda: bool(w.visible("set_title")), 6))
        L.close_settings(w)
        w.click("sb_theme_hit")
        w.check("rail: back to Light", w.logged("theme -> light", 6))
        w.click("rail_expand")
        w.check("the column expands back with its labels", w.wait(lambda: bool(w.visible("sb_theme_label")), 6))
    sys.exit(w.summary())


if __name__ == "__main__":
    main()
