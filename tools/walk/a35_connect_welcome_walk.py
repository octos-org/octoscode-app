#!/usr/bin/env python3
"""Check the Mac first-run connection view and its pairing return path.

The walk runner supplies an isolated local shell. No connection is attempted.
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from walk_env import App  # noqa: E402

WALK = {
    "name": "a35_connect_welcome",
    "title": "Mac first-run connection view remains usable through pairing",
    "modes": ["desktop"],
    "app": {"env": {"OCTOSCODE_NO_PORT": "1", "OCTOSCODE_FIRST_RUN": "1",
                     "HEADLESS_ARGS": "--module octoscode --test-action style:macos"},
            "ready": ["connect_card", "connect_btn"]},
    "runs": [{"argv": ["{port}"]}],
    "timeout": 120,
    "rows": {},
}


def inside(inner, outer):
    return (inner[0] >= outer[0] - 1 and inner[1] >= outer[1] - 1
            and inner[0] + inner[2] <= outer[0] + outer[2] + 1
            and inner[1] + inner[3] <= outer[1] + outer[3] + 1)


def main():
    app = App(int(sys.argv[1]))
    passed = []

    def check(name, value):
        passed.append(bool(value))
        print(("PASS " if value else "FAIL ") + name, flush=True)
        return bool(value)

    if not check("the connection form is ready", app.wait(lambda: app.find("connect_card"), timeout=30)):
        return 1
    rows = app.snap()
    views = [row for row in rows if row.get("ty") == "MpModuleView" and App.shown(row)]
    window = max(views, key=lambda row: row["r"][2] * row["r"][3])["r"] if views else app.find("main_window", rows)["r"]
    card = app.find("connect_card", rows)["r"]
    check("the welcome content is centred below the toolbar",
          app.find("first_run_mark", rows) and inside(card, window)
          and card[1] > window[1] + 80
          and abs(card[0] + card[2] / 2 - window[0] - window[2] / 2) <= 2)
    controls = [app.find(wid, rows) for wid in ("connect_title", "connect_server", "connect_token",
                                                 "b1_connect_pair", "connect_solo", "connect_btn")]
    if not check("the fields and actions fit in the welcome content",
                 all(row and inside(row["r"], card) for row in controls)):
        return 1

    def replace_address(value):
        app.click("connect_server")
        app.get("/k?c=KeyA&cmd=1&wait=1")
        app.type(value)

    replace_address("not an address")
    check("an invalid address disables Connect", app.wait(lambda: app.find("connect_btn_label_off"), timeout=8))
    address = "http://localhost:18032"
    replace_address(address)
    if not check("a valid address enables Connect", app.wait(lambda: app.find("connect_btn_label"), timeout=8)):
        return 1
    if not check("the pairing form opens", app.click("b1_connect_pair") and app.wait(lambda: app.find("b1_pair_link"), timeout=12)):
        return 1
    app.click("b1_pair_back")
    returned = app.wait(lambda: not app.find("b1_pair_link") and app.find("connect_server"), timeout=12)
    check("cancelling pairing retains the address", returned and app.text("connect_server") == address)
    return 0 if all(passed) else 1


if __name__ == "__main__":
    raise SystemExit(main())
