#!/usr/bin/env python3
"""Overlay layers over full-window surfaces (issue #28): a layer the person
opens while Fleet, Session history or a dialog is up must be drawn, and take
presses, ABOVE it — never left underneath.

The module root is an Overlay: a later sibling paints (and is hit) above an
earlier one, and /snap lists the widgets in that tree order, so "the dock's
index is above every shown surface dock's" is the paint order itself. Each
layer is also CLICKED at its laid-out centre and must answer, which proves the
press reached it and not the surface beneath.

The rule under test:
  * popups — the command palette, the Stop confirm, the sidebar menus — paint
    above every surface and take Escape first, leaving the surface open;
  * Settings is a surface: opening it replaces the Fleet pane (its sidebar
    entry stays clickable next to it), and the surfaces Settings opens itself
    (Capabilities, Models) sit above it;
  * a modal surface (a board-3 dialog, an A5 dialog) owns the pointer and the
    keyboard, so Settings and the workspace menu are not reachable under it
    (nothing is opened invisibly); the review panel is a conversation panel
    the surfaces cover, not a layer over them.

usage: OCTOSCODE_APP_BIN=<octoscode binary> overlay_layers_walk.py <desktop> <outdir>
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from a10_lib import Walk, run_session  # noqa: E402  (a10_lib imports bridgeauth: D10c)

# A11: the walk aggregator's convention (tools/walk/native.py; read with ast,
# never imported). run_session takes the aggregator's ports.
WALK = {
    "name": "overlay_layers",
    "title": "Layers over full-window surfaces: Settings, menus, palette, Stop confirm paint above Fleet and dialogs",
    "modes": ["desktop"],
    "app": "self",
    "runs": [{"argv": ["{mode}", "{out}"], "env": {"A10_PORT": "{port}", "A10_REPLAY_PORT": "{fport}"}}],
    "needs": ["target/debug/examples/replay_serve"],
    "timeout": 600,
    "rows": {},
}

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else "tmp/overlay_layers"

# The full-window surface docks of the module root, bottom to top.
SURFACES = ("screen_dock", "fleet_dock", "dialog_dock", "board1_dock", "board3_dock", "surfaces_dock", "a9_dock")
DOCKS = SURFACES + ("settings_dock", "stop_dock", "sb_menu_dock", "palette_dock", "toast_dock")


def order(sn) -> dict:
    """Each dock's position in the snap's tree order (= its paint order)."""
    idx: dict = {}
    for i, w in enumerate(sn):
        if w.get("i") in DOCKS and w["i"] not in idx:
            idx[w["i"]] = i
    return idx


def shown_docks(W: Walk, sn=None) -> list:
    sn = sn if sn is not None else W.snap()
    return [w["i"] for w in sn if w.get("i") in DOCKS and W.shown(w)]


def above_all_surfaces(W: Walk, dock: str, probe: str):
    """(ok, detail): `dock` and its `probe` widget are shown and no shown
    surface dock is drawn after (above) it."""
    sn = W.snap()
    idx, shown = order(sn), shown_docks(W, sn)
    over = [d for d in shown if d in SURFACES and idx[d] > idx.get(dock, 10**9)]
    r = W.rect(probe, sn=sn)
    ok = dock in shown and r is not None and not over
    return ok, f"{dock} idx={idx.get(dock)} {probe}={r} surfaces above it={over} shown={shown}"


def reset(W: Walk) -> None:
    for _ in range(6):
        if not [d for d in shown_docks(W) if d != "toast_dock"]:
            break
        W.key("Escape")
    time.sleep(0.3)


def open_fleet(W: Walk) -> bool:
    W.click("fleet_nav_hit")
    return W.wait_shown("b3_fleet_panel", 10)


def open_history(W: Walk) -> bool:
    W.click("history_nav_hit")
    return W.wait_shown("b3_dialog", 10)


def open_models(W: Walk) -> bool:
    return W.palette_run("mo", "/model") and W.wait_shown("dlg_models_card_deepseek", 10)


def palette_over(W: Walk, base: str, base_dock: str, still_open) -> None:
    """Cmd+K over the surface: the palette is above it, answers a click, and
    Escape closes it first (the surface stays), then the surface."""
    W.get("/k?c=KeyK&cmd=1&wait=1", tolerant=True)
    W.wait_shown("palette", 6)
    ok, detail = above_all_surfaces(W, "palette_dock", "palette")
    W.check(f"{base}: the command palette is above the surface", ok, detail)
    W.type_text("mo")
    rows = W.wait(lambda: any((w.get("t") or "") == "/model" for w in W.visible("palette_row_name")), 6)
    W.check(f"{base}: the palette lists commands (its field takes the keys)", rows)
    W.key("Escape")
    W.check(f"{base}: Escape closes the palette first", W.wait_shown("palette", 4, gone=True) and still_open(W))
    W.key("Escape")
    W.check(f"{base}: the next Escape closes the surface", W.wait(lambda: base_dock not in shown_docks(W), 6))


def chat_menu_over(W: Walk, base: str, base_dock: str, still_open) -> None:
    """A right-click on a chat row (near the sidebar's right edge): the menu is
    above the surface and Escape closes it first (the surface stays); its
    Rename answers a click and the inline rename's Escape leaves the surface."""
    for step in ("Escape", "Rename"):
        r = W.rect("sb_r_open", 0)
        if not r:
            W.check(f"{base}: a chat row to right-click", False)
            return
        W.get(f"/m?k=click&x={r[0] + r[2] - 30:.0f}&y={r[1] + r[3] / 2:.0f}&b=1&wait=1", tolerant=True)
        W.wait_shown("sb_chat_rename", 6)
        if step == "Escape":
            ok, detail = above_all_surfaces(W, "sb_menu_dock", "sb_chat_menu")
            W.check(f"{base}: the chat row's context menu is above the surface", ok, detail)
            W.shot(f"{base}_chat_menu")
        else:
            W.mark()
            W.click("sb_chat_rename")
            W.check(f"{base}: the menu's Rename takes the click", W.logged("session.rename", 5))
        W.key("Escape")
        W.check(f"{base}: Escape after the menu{' / the rename' if step == 'Rename' else ''} leaves the surface open",
                W.wait(lambda: "sb_menu_dock" not in shown_docks(W), 4) and still_open())


def walk(W: Walk) -> None:
    W.note("== 1. over the Fleet pane (the sidebar stays clickable next to it)")
    fleet_up = lambda W=W: bool(W.visible("b3_fleet_panel"))
    W.check("fleet: the Fleet pane opens", open_fleet(W))
    W.click("sb_settings_hit")
    W.check("fleet: Settings opens above it (the pane is replaced)",
            W.wait_shown("settings_drawer", 6) and above_all_surfaces(W, "settings_dock", "settings_drawer")[0]
            and not W.visible("b3_fleet_panel"),
            above_all_surfaces(W, "settings_dock", "settings_drawer")[1])
    W.shot("fleet_settings")
    W.mark()
    W.click("nv_hit", 1)
    W.check("fleet: a click in Settings reaches Settings", W.logged("settings.section.permissions", 5))
    W.click("nv_hit", 0)
    W.click("server_stop_request")
    W.check("fleet: the Stop confirm opens above Settings", W.wait_shown("stop_dialog", 6)
            and above_all_surfaces(W, "stop_dock", "stop_dialog")[0],
            above_all_surfaces(W, "stop_dock", "stop_dialog")[1])
    W.shot("fleet_settings_stop")
    W.mark()
    W.click("server_stop_cancel")
    W.check("fleet: Cancel on the Stop confirm takes the click", W.logged("server.stop.cancel", 5)
            and W.wait_shown("stop_dialog", 4, gone=True))
    W.key("Escape")
    W.check("fleet: Escape closes Settings", W.wait_shown("settings_drawer", 6, gone=True))
    reset(W)

    W.check("fleet: the Fleet pane reopens", open_fleet(W))
    chat_menu_over(W, "fleet", "board3_dock", fleet_up)
    W.click("sb_g_more", 0)
    W.check("fleet: the workspace menu is above the pane", W.wait_shown("sb_menu_items", 6)
            and above_all_surfaces(W, "sb_menu_dock", "sb_menu")[0], above_all_surfaces(W, "sb_menu_dock", "sb_menu")[1])
    W.mark()
    W.click("sb_menu_rename")
    W.check("fleet: the workspace menu's Rename takes the click", W.wait_shown("sb_rename_form", 6)
            and W.logged("workspace.rename", 5))
    W.key("Escape")
    W.check("fleet: Escape closes the workspace menu, the pane stays",
            W.wait(lambda: "sb_menu_dock" not in shown_docks(W), 4) and fleet_up())
    palette_over(W, "fleet", "board3_dock", fleet_up)
    reset(W)

    W.note("== 2. over Session history (a board-3 screen: a modal dialog)")
    history_up = lambda W=W: bool(W.visible("b3_dialog"))
    W.check("history: Session history opens", open_history(W))
    chat_menu_over(W, "history", "board3_dock", history_up)
    W.click("sb_settings_hit")
    W.check("history: Settings is not opened under the modal dialog",
            "settings_dock" not in shown_docks(W) and history_up())
    W.click("sb_g_more", 0)
    W.check("history: the workspace menu is not opened under the modal dialog",
            "sb_menu_dock" not in shown_docks(W) and history_up())
    palette_over(W, "history", "board3_dock", history_up)
    reset(W)

    W.note("== 3. over an A5 dialog (Models)")
    models_up = lambda W=W: bool(W.visible("dlg_models_card_deepseek"))
    W.check("models: the Models dialog opens", open_models(W))
    chat_menu_over(W, "models", "dialog_dock", models_up)
    W.get("/k?c=KeyK&cmd=1&wait=1", tolerant=True)
    W.check("models: the dialog owns the keyboard (no palette opens under it)",
            "palette_dock" not in shown_docks(W) and models_up())
    reset(W)

    W.note("== 4. a surface Settings opens sits above Settings")
    W.click("sb_settings_hit")
    W.wait_shown("settings_drawer", 6)
    W.click("nv_hit", 3)
    W.check("settings: Capabilities opens", W.wait_shown("set_cap_tools", 6))
    W.click("set_cap_tools")
    W.wait_shown("b3_dialog", 8)
    sn = W.snap()
    idx, shown = order(sn), shown_docks(W, sn)
    W.check("settings: the Tools inventory opens above Settings",
            "settings_dock" in shown and "board3_dock" in shown and idx["board3_dock"] > idx["settings_dock"],
            f"settings_dock={idx.get('settings_dock')} board3_dock={idx.get('board3_dock')} shown={shown}")
    W.shot("settings_tools")
    W.key("Escape")
    W.check("settings: Escape closes the inventory first, Settings stays",
            W.wait(lambda: "board3_dock" not in shown_docks(W), 6) and bool(W.visible("settings_drawer")))
    W.click("nv_hit", 2)
    W.wait_shown("set_models_manage", 6)
    W.click("set_models_manage")
    W.wait(lambda: any(d in SURFACES for d in shown_docks(W)), 8)
    sn = W.snap()
    idx = order(sn)
    opened = [d for d in shown_docks(W, sn) if d in SURFACES]
    W.check("settings: Manage providers opens above Settings",
            bool(opened) and all(idx[d] > idx["settings_dock"] for d in opened), f"{opened} {idx}")
    W.shot("settings_providers")
    reset(W)
    W.check("settings: everything closes", not [d for d in shown_docks(W) if d != "toast_dock"])


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, scenario="a10"))
