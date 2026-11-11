#!/usr/bin/env python3
"""Check phone Settings secondary text and selected controls from pixels.

The native walk runner supplies an isolated shell and a local replay fixture.
The frame is a desktop-hosted phone preview, not a physical-device test.
"""
import io
import json
import pathlib
import sys
import time

from PIL import Image

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from walk_env import App  # noqa: E402

WALK = {
    "name": "a34_phone_settings_contrast",
    "title": "Phone preview: Settings remains readable and shows its selected option",
    "modes": ["phone"],
    "fixture": {"argv": ["{examples}/replay_serve", "{fport}", "--scenario", "history",
                         "--demo", "{root}/tools/walk/fixtures/phone-preview-demo.json",
                         "--root", "/Users/demo/Projects/lumen"]},
    "app": {"env": {"OCTOS_BASE_URL": "http://127.0.0.1:{fport}",
                    "OCTOS_PROFILE_ID": "dsflash", "OCTOSCODE_NO_PORT": "1",
                    "OCTOS_WORKSPACE_CWD": "/Users/demo/Projects/lumen"},
            "ready": ["i0_composer_0"]},
    "runs": [{"argv": ["{port}"]}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 210,
    "rows": {},
}


def luminance(color):
    def linear(value):
        value /= 255
        return value / 12.92 if value <= .04045 else ((value + .055) / 1.055) ** 2.4
    return sum(weight * linear(value) for weight, value in zip((.2126, .7152, .0722), color))


def frame(app):
    time.sleep(2)
    rows = app.snap()
    size = json.loads(app.get("/s"))["w"][0]["sz"]
    image = Image.open(io.BytesIO(app.png())).convert("RGB")
    return rows, image, image.width / size[0]


def ratio(image, scale, rect):
    x, y, w, h = rect
    box = tuple(round(value * scale) for value in (x, y, x + w, y + h))
    bg = image.getpixel((round((x + w + 3) * scale), round((y + h / 2) * scale)))
    bg_lum = luminance(bg)
    ink = max(image.crop(box).getdata(), key=lambda color: abs(luminance(color) - bg_lum))
    high, low = sorted((luminance(ink), bg_lum), reverse=True)
    return (high + .05) / (low + .05)


def ratio_on(image, scale, rect, bg):
    x, y, w, h = rect
    box = tuple(round(value * scale) for value in (x, y, x + min(w, 120), y + h))
    bg_lum = luminance(bg)
    ink = max(image.crop(box).getdata(), key=lambda color: abs(luminance(color) - bg_lum))
    high, low = sorted((luminance(ink), bg_lum), reverse=True)
    return (high + .05) / (low + .05)


def label_background(image, scale, rect):
    x, y, w, h = rect
    return image.getpixel((round((x + w + 3) * scale), round((y + h / 2) * scale)))


def icon_contrast(image, scale, rect, bg):
    x, y, w, h = rect
    # Exclude the button edge: only the glyph should make the check pass.
    box = tuple(round(value * scale) for value in (x + 5, y + 5, x + w - 5, y + h - 5))
    bg_lum = luminance(bg)
    ratios = []
    for color in image.crop(box).getdata():
        high, low = sorted((luminance(color), bg_lum), reverse=True)
        ratios.append((high + .05) / (low + .05))
    return sum(value >= 3 for value in ratios)


def main():
    app = App(int(sys.argv[1]))
    results = []

    def check(name, value):
        results.append(bool(value))
        print(("PASS " if value else "FAIL ") + name, flush=True)
        return bool(value)

    if not check("the conversation is ready", app.wait(lambda: app.find("i0_composer_0"), timeout=30)):
        return 1
    if not check("Settings opens", app.click("settings_open_hit") and
                 app.wait(lambda: app.find("settings_drawer"), timeout=10)):
        return 1
    rows, image, scale = frame(app)
    labels = ("Desktop notifications are unavailable here.", "Connected",
              "The Octos profile backing this session.")
    first_label = next(row["r"] for row in rows if row.get("t") == labels[0])
    system_background = label_background(image, scale, first_label)
    minimum = min(ratio(image, scale, next(row["r"] for row in rows if row.get("t") == label)) for label in labels)
    print(f"Minimum Settings secondary-text contrast: {minimum:.2f}:1", flush=True)
    check("Settings secondary text has at least 4.5:1 contrast", minimum >= 4.5)
    if not check("Model settings opens", app.click("set_rail_model") and
                 app.wait(lambda: app.find("sec_model") and app.find("th_high"), timeout=10)):
        return 1
    if not check("the High option can be selected", app.click("th_high") and
                 app.wait(lambda: any(row.get("i") == "sg_text_on" and row.get("t") == "High"
                                      for row in app.snap()), timeout=10)):
        return 1
    rows, image, scale = frame(app)
    selected = app.find("th_high", rows)["r"]
    other = app.find("th_off", rows)["r"]
    point = lambda r: image.getpixel((round((r[0] + 6) * scale), round((r[1] + r[3] / 2) * scale)))
    difference = max(abs(a - b) for a, b in zip(point(selected), point(other)))
    print(f"Selected segment background difference: {difference}", flush=True)
    check("the selected model option has a visible background", difference >= 12)
    check("the Off option can be selected", app.click("th_off") and
          app.wait(lambda: any(row.get("i") == "sg_text_on" and row.get("t") == "Off"
                               for row in app.snap()), timeout=10))
    if not check("Preferences opens", app.click("set_rail_preferences") and
                 app.wait(lambda: app.find("pal_solarized"), timeout=10)):
        return 1
    if not check("the named palette can be selected", app.click("pal_solarized") and
                 app.wait(lambda: any("theme -> solarized" in line for line in app.all_logs()), timeout=15)):
        return 1
    if not check("General remains reachable", app.click("set_rail_general") and
                 app.wait(lambda: app.find("notify_help"), timeout=10)):
        return 1
    rows, image, scale = frame(app)
    named_background = label_background(image, scale, next(row["r"] for row in rows if row.get("t") == labels[0]))
    print(f"System and named-palette backgrounds: {system_background}, {named_background}", flush=True)
    check("Solarized is applied to the Settings surface",
          max(abs(a - b) for a, b in zip(named_background, (0, 43, 54))) <= 8)
    named_minimum = min(ratio(image, scale, next(row["r"] for row in rows if row.get("t") == label))
                        for label in labels)
    print(f"Named-palette Settings contrast: {named_minimum:.2f}:1", flush=True)
    check("named-palette Settings text remains readable", named_minimum >= 4.5)
    if not check("the sidebar opens after the theme change", app.click("set_back") and
                 app.wait(lambda: not app.find("settings_drawer"), timeout=10) and
                 app.click("sidebar_toggle_hit") and app.wait(lambda: app.find("sidebar_brand_name"), timeout=10)):
        return 1
    rows, image, scale = frame(app)
    selected = app.find("sb_r_sel", rows)["r"]
    title = next(row["r"] for row in app.all("sb_r_title", rows)
                 if selected[1] <= row["r"][1] < selected[1] + selected[3])
    date = next(row["r"] for row in app.all("sb_r_time", rows)
                if selected[1] <= row["r"][1] < selected[1] + selected[3])
    x, y, _, h = selected
    bg = image.getpixel((round((x + 17) * scale), round((y + h / 2) * scale)))
    selected_minimum = min(ratio_on(image, scale, title, bg), ratio_on(image, scale, date, bg))
    print(f"Named-palette selected conversation contrast: {selected_minimum:.2f}:1", flush=True)
    check("named-palette selected conversation has a contrast margin", selected_minimum >= 5.0)
    close = app.find("drawer_close", rows)["r"]
    side_bg = image.getpixel((round((close[0] - 4) * scale), round((close[1] + close[3] / 2) * scale)))
    visible_pixels = icon_contrast(image, scale, close, side_bg)
    print(f"Sidebar close-icon pixels with at least 3:1 contrast: {visible_pixels}", flush=True)
    check("the named-palette sidebar close icon is visible", visible_pixels >= 6)
    if not check("the conversation remains reachable", app.click("drawer_close") and
                 app.wait(lambda: app.find("i0_composer_0") and
                          any("Two small things, neither of them urgent" in (row.get("t") or "")
                              for row in app.snap()), timeout=10)):
        return 1
    if not check("Preferences can be reopened", app.click("settings_open_hit") and
                 app.wait(lambda: app.find("settings_drawer"), timeout=10) and
                 app.click("set_rail_preferences") and app.wait(lambda: app.find("pal_system"), timeout=10)):
        return 1
    if not check("System can be restored", app.click("pal_system") and
                 app.wait(lambda: any("a9 prefs: palette terminal" in line for line in app.all_logs()), timeout=15) and
                 app.click("set_rail_general") and app.wait(lambda: app.find("notify_help"), timeout=10)):
        return 1
    rows, image, scale = frame(app)
    restored = label_background(image, scale, next(row["r"] for row in rows if row.get("t") == labels[0]))
    restored_minimum = min(ratio(image, scale, next(row["r"] for row in rows if row.get("t") == label))
                           for label in labels)
    print(f"Restored System background and minimum contrast: {restored}, {restored_minimum:.2f}:1", flush=True)
    check("System restores the host surface", max(abs(a - b) for a, b in zip(restored, system_background)) <= 8)
    check("restored System text remains readable", restored_minimum >= 4.5)
    for choice in ("dark", "light", "system"):
        if not check(f"Preferences opens for {choice}", app.click("set_rail_preferences") and
                     app.wait(lambda: app.find("pal_" + choice), timeout=10)):
            return 1
        action = "a9.prefs.palette." + choice
        before = sum(action in line for line in app.all_logs())
        if not check(f"{choice} theme can be selected", app.click("pal_" + choice) and
                     app.wait(lambda: sum(action in line for line in app.all_logs()) > before, timeout=15) and
                     app.click("set_rail_general") and app.wait(lambda: app.find("notify_help"), timeout=10)):
            return 1
        rows, image, scale = frame(app)
        bg = label_background(image, scale, next(row["r"] for row in rows if row.get("t") == labels[0]))
        minimum = min(ratio(image, scale, next(row["r"] for row in rows if row.get("t") == label))
                      for label in labels)
        expected_dark = luminance(system_background) < .12 if choice == "system" else choice == "dark"
        print(f"{choice} background and minimum contrast: {bg}, {minimum:.2f}:1", flush=True)
        check(f"{choice} applies the selected appearance", luminance(bg) < .12 if expected_dark else luminance(bg) > .6)
        check(f"{choice} Settings text remains readable", minimum >= 4.5)
        if choice in ("dark", "light"):
            if not check(f"{choice} sidebar opens", app.click("set_back") and
                         app.wait(lambda: not app.find("settings_drawer"), timeout=10) and
                         app.click("sidebar_toggle_hit") and
                         app.wait(lambda: app.find("sidebar_brand_name"), timeout=10)):
                return 1
            rows, image, scale = frame(app)
            selected = app.find("sb_r_sel", rows)["r"]
            dates = app.all("sb_r_time", rows)
            selected_date = next(row["r"] for row in dates
                                 if selected[1] <= row["r"][1] < selected[1] + selected[3])
            other_date = next(row["r"] for row in dates if row["r"][1] >= selected[1] + selected[3])
            x, y, _, h = selected
            chosen = image.getpixel((round((x + 17) * scale), round((y + h / 2) * scale)))
            other = image.getpixel((round((x + 17) * scale), round((other_date[1] + other_date[3] / 2) * scale)))
            difference = max(abs(a - b) for a, b in zip(chosen, other))
            date_ratio = ratio_on(image, scale, selected_date, chosen)
            print(f"{choice} selection difference and date contrast: {difference}, {date_ratio:.2f}:1", flush=True)
            check(f"{choice} shows the selected conversation", difference >= 12)
            check(f"{choice} selected date remains readable", date_ratio >= 4.5)
            if not check("Settings reopens", app.click("drawer_close") and
                         app.wait(lambda: not app.find("sidebar_brand_name"), timeout=10) and
                         app.click("settings_open_hit") and app.wait(lambda: app.find("settings_drawer"), timeout=10) and
                         app.click("set_rail_general") and app.wait(lambda: app.find("notify_help"), timeout=10)):
                return 1
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
