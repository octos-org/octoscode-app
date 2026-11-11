#!/usr/bin/env python3
"""Exercise Back and the composer in the hosted phone-sized preview.

The native walk runner starts a local replay server and an isolated shell.
This checks the simulated keyboard; it is not a physical-device test.
"""
import io
import json
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from walk_env import App  # noqa: E402

WALK = {
    "name": "a34_phone_palette_back",
    "title": "Phone preview: Back, keyboard and style changes preserve the conversation",
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
    "timeout": 120,
    "rows": {},
}


def sidebar_pixels(app):
    """Measure the shown text and selection on the actual phone-preview frame."""
    from PIL import Image  # The native visual walks use Pillow for pixel checks.

    def luminance(color):
        def linear(value):
            value /= 255
            return value / 12.92 if value <= .04045 else ((value + .055) / 1.055) ** 2.4
        return sum(weight * linear(value) for weight, value in zip((.2126, .7152, .0722), color))

    def contrast(image, scale, rect, search=False):
        x, y, w, h = rect
        if search:
            box = (x + 34, y + 6, x + 148, y + h - 6)
            sample = (x + w - 25, y + h / 2)
        else:
            box = (x, y, x + w, y + h)
            sample = (x + w + 3, y + h / 2)
        box = tuple(round(value * scale) for value in box)
        bg = image.getpixel(tuple(round(value * scale) for value in sample))
        bg_lum = luminance(bg)
        ink = max(image.crop(box).getdata(), key=lambda color: abs(luminance(color) - bg_lum))
        high, low = sorted((luminance(ink), bg_lum), reverse=True)
        return (high + .05) / (low + .05)

    rows = app.snap()
    window = app.find("main_window", rows)["r"]
    selected = app.find("sb_r_sel", rows)["r"]
    dates = app.all("sb_r_time", rows)
    selected_date = next(row for row in dates if selected[1] <= row["r"][1] < selected[1] + selected[3])
    other_date = next(row for row in dates if row["r"][1] >= selected[1] + selected[3])
    inactive = next(row for row in app.all("sg_text_off", rows) if row.get("t") == "All")
    search = app.find("sb_search", rows)
    brand = app.find("sidebar_brand_name", rows)["r"]
    for _ in range(3):
        data = app.png()
        if not data:
            continue
        image = Image.open(io.BytesIO(data)).convert("RGB")
        scale = image.width / window[2]
        # A hidden grab can arrive before the full frame. Retry only when the
        # brand itself is absent; a fully drawn but faint label still fails.
        if contrast(image, scale, [brand[0], brand[1], min(brand[2], 115), brand[3]]) >= 4.5:
            break
        time.sleep(.5)
    else:
        raise RuntimeError("the sidebar frame did not show its brand")
    x, y, _, h = selected
    point = lambda px, py: image.getpixel((round(px * scale), round(py * scale)))
    chosen_bg = point(x + 17, y + h / 2)
    other_bg = point(x + 17, other_date["r"][1] + other_date["r"][3] / 2)
    difference = max(abs(a - b) for a, b in zip(chosen_bg, other_bg))
    ratios = [contrast(image, scale, row["r"]) for row in (selected_date, other_date, inactive)]
    ratios.append(contrast(image, scale, search["r"], search=True))
    return difference, min(ratios)


def main():
    app = App(int(sys.argv[1]))
    dark = "--dark" in sys.argv[2:]
    results = []

    def check(name, value):
        results.append(bool(value))
        print(("PASS " if value else "FAIL ") + name, flush=True)
        return bool(value)

    if not check("the phone composer is ready", app.wait(lambda: app.find("i0_composer_0"), timeout=30)):
        return 1
    def conversation_visible():
        return any("Two small things, neither of them urgent" in (row.get("t") or "")
                   for row in app.snap())

    if not check("the synthetic conversation is visible", app.wait(conversation_visible, timeout=15)):
        return 1
    app.click("i0_composer_0")
    app.type("/")
    if not check("typing slash opens the command palette", app.wait(lambda: app.find("palette_list"), timeout=10)):
        return 1
    app.key("Escape")  # The phone shell dismisses the simulated keyboard first.
    time.sleep(0.6)
    if not check("the first Back leaves the palette open", app.find("palette_list")):
        return 1
    app.key("Escape")
    if not check("the second Back dismisses the palette", app.wait(lambda: not app.find("palette_list"), timeout=10)):
        return 1
    if not check("Back keeps the unsent slash", app.text("i0_composer_0") == "/"):
        return 1
    app.click("i0_composer_0")
    app.type("draft")
    if not check("the same conversation accepts more draft text",
                 app.wait(lambda: app.text("i0_composer_0") == "/draft", timeout=10)):
        return 1
    time.sleep(2)  # Measure the settled keyboard, not its opening animation.
    size = json.loads(app.get("/s"))["w"][0]["sz"]
    rect = app.find("i0_composer_0")["r"]
    keyboard_top = size[1] - 316  # 292-point simulated keyboard + 24-point navigation strip.
    check("the composer is above the simulated keyboard", rect[1] + rect[3] <= keyboard_top + 1)
    app.key("Escape")
    time.sleep(2)
    rect = app.find("i0_composer_0")["r"]
    check("dismissing the keyboard restores the layout and draft",
          rect[1] > keyboard_top and app.text("i0_composer_0") == "/draft")
    # Entering phone style returns to Home. Reopen the existing app after the
    # roundtrip; the test events invoke the shell's normal menu actions.
    app.get("/event?data=mac-row%3Adesktop." + ("macos-dark" if dark else "macos"))
    def desktop_has_size():
        size = json.loads(app.get("/s"))["w"][0]["sz"]
        return size[0] > 600 and size[1] > 400

    if not check("desktop style restores a usable window", app.wait(desktop_has_size, timeout=20)):
        return 1
    if not check("the desktop copy label fits on one line",
                 app.wait(lambda: app.find("hd_copy_label") and app.find("hd_copy_label")["r"][3] <= 18,
                          timeout=20)):
        return 1
    check("the phone sidebar brand is hidden on desktop",
          not app.find("sidebar_brand_icon") and not app.find("sidebar_brand_name"))
    app.get("/event?data=mac-row%3Adesktop." + ("ios-dark" if dark else "ios"))
    time.sleep(3)
    app.get("/event?data=mac-row%3Aapps.octoscode")
    if not check("the conversation remains visible after changing styles",
                 app.wait(conversation_visible, timeout=20)):
        return 1
    check("changing styles keeps the draft", app.wait(lambda: app.text("i0_composer_0") == "/draft", timeout=10))
    if not check("the phone sidebar opens", app.click("sidebar_toggle_hit") and
                 app.wait(lambda: app.find("sidebar_brand_name"), timeout=10)):
        return 1
    time.sleep(2)
    difference, ratio = sidebar_pixels(app)
    print(f"Sidebar selection difference: {difference}; minimum text contrast: {ratio:.2f}:1", flush=True)
    check("the selected conversation has a visible background", difference >= 12)
    check("sidebar secondary text has at least 4.5:1 contrast", ratio >= 4.5)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
