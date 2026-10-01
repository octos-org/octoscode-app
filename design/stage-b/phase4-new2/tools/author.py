#!/usr/bin/env python3
"""Author board 2 (design/stage-a/phase4-new2) screens 1-5 contracts — the
D2a sidebar half. Modeled on the autonomy board's tools/author.py (same
contract schema, palette, fonts, Scene/controls shapes), with one honest
difference: there is no external OCR for this atlas, so the "observations"
are the lane model's OWN vision measurements from the atlas crops
(tmp/d2a/*.png, cut on the measured hairline grid), rescaled to the shared
406x776 artboard. Coordinates are authored in artboard space directly.

Screens (operator-approved atlas-prompt.md):
  01 grouped sidebar  02 statuses  03 search  04 collapsed rail  05 drawer

Run: python3 design/stage-b/phase4-new2/tools/author.py
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# The atlas prompt's palette (the same token set the autonomy author uses).
C = {"white": 0xFFFFFFFF, "panel": 0xFFF7F7F8, "hair": 0xFFE5E5E7, "ink": 0xFF1D1D1F,
     "muted": 0xFF6E6E73, "black": 0xFF000000, "blue": 0xFF2F6FEB, "amber": 0xFFB45309,
     "green": 0xFF1F883D, "red": 0xFFCF222E, "sel": 0xFFF2F2F7, "chip": 0xFFEFEFF0,
     "hl": 0xFFFCE9B8, "dim": 0x99000000}
C_HEX = {k: f"#{v & 0xFFFFFF:06X}" for k, v in C.items()}
FONT = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf",
        600: "self:resources/ux/Inter-600.ttf", 700: "self:resources/ux/Inter-700.ttf"}

TITLES = {1: "Sidebar grouped", 2: "Sidebar statuses", 3: "Sidebar search",
          4: "Workspace collapsed", 5: "Compact drawer"}
SOURCE = {n: f"phase4-new2-screen-{n:02d}" for n in range(1, 6)}

# The fixture copy the atlas prompt pins (titles/times as seen on the crops).
ROWS = {
    "octos": [("Fix steer queue drop on reconnect", "2m"),
              ("Add session fork", "1h"),
              ("Review PR #2566", "Yesterday")],
    "octoscode-app": [("Bump octos-core to a6ea8505", "2h"),
                      ("Why is hydrate slow?", "Yesterday")],
}
STATUSES = [("Running", "2m", "blue-dot"), ("Waiting for input", "1h", "amber-dot"),
            ("Done", "Yesterday", "check"), ("Failed", "2h", "red-dot"),
            ("Idle", "Yesterday", "hollow")]


def hexpal():
    return {v: f"#{v & 0xFFFFFF:06X}" for v in set(C.values())}


def svg(name, color="#1D1D1F"):
    P = {
        "pencil": '<path d="M3 17.2V21h3.8L17.8 10 14 6.2 3 17.2zM20.7 7.3c.4-.4.4-1 0-1.4l-2.6-2.6c-.4-.4-1-.4-1.4 0l-1.8 1.8L18.9 8l1.8-1.7z"/>',
        "chev-down": '<path d="M6 9l6 6 6-6" fill="none" stroke="CURRENT" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>',
        "chev-right": '<path d="M9 6l6 6-6 6" fill="none" stroke="CURRENT" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>',
        "search": '<circle cx="10.5" cy="10.5" r="6" fill="none" stroke="CURRENT" stroke-width="1.6"/><path d="M15 15l5 5" stroke="CURRENT" stroke-width="1.6" stroke-linecap="round"/>',
        "plus": '<path d="M12 5v14M5 12h14" fill="none" stroke="CURRENT" stroke-width="1.8" stroke-linecap="round"/>',
        "close": '<path d="M6 6l12 12M18 6L6 18" stroke="CURRENT" stroke-width="1.8" stroke-linecap="round"/>',
        "check": '<path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="CURRENT" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>',
        "dots": '<circle cx="6" cy="12" r="1.5"/><circle cx="12" cy="12" r="1.5"/><circle cx="18" cy="12" r="1.5"/>',
        "dot": '<circle cx="12" cy="12" r="5"/>',
        "hollow": '<circle cx="12" cy="12" r="5" fill="none" stroke="CURRENT" stroke-width="1.8"/>',
    }
    body = P.get(name, P["dot"]).replace("CURRENT", color)
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" '
            f'fill="{color if name in ("pencil","dots","dot") else "none"}">{body}</svg>')


ICON_REG = {}


def icon(iid, name, x, y, w, h, color="ink"):
    ICON_REG[iid] = (name, C.get(color, color))
    return {"id": iid, "type": "svg", "x": x, "y": y, "w": w, "h": h,
            "svg": name, "color": C_HEX.get(color, color)}


def text(iid, s, x, y, w, h, *, weight=400, color="ink", size=14):
    return {"id": iid, "type": "text", "x": x, "y": y, "w": w, "h": h, "text": s,
            "font": FONT[weight], "size": size, "color": C_HEX.get(color, C["ink"])}


def surface(iid, x, y, w, h, *, bg="white", radius=12, border=0, kids=None):
    n = {"id": iid, "type": "surface", "x": x, "y": y, "w": w, "h": h,
         "bg": C_HEX.get(bg, bg), "radius": radius}
    if border:
        n["border"] = {"color": C_HEX["hair"], "width": 1}
    if kids:
        n["kids"] = kids
    return n


class Scene:
    """Index-row composition on the 406x776 artboard (the autonomy pattern)."""

    def __init__(self, num):
        self.num = num
        self.kids = []
        self.controls = {}
        self.inputs = {}
        self.obs = []

    def put(self, node):
        self.kids.append(node)

    def control(self, cid, event, bounds, *, enabled=True):
        self.controls[cid] = {"event": event, "source_bounds": list(bounds), "enabled": enabled}

    def observe(self, s, x, y, w, h):
        self.obs.append({"text": s, "bounds": [x, y, w, h], "confidence": 1,
                         "source": "lane-vision"})


def header(sc, *, search_text="Search chats", clear=False):
    sc.put(text("t_head", "OctosCode", 20, 18, 200, 24, weight=700, size=19))
    sc.observe("OctosCode", 20, 18, 200, 24)
    sc.put(icon("i_pencil", "pencil", 22, 56, 18, 18))
    sc.put(text("t_newchat", "New chat", 48, 54, 120, 22, weight=500))
    sc.control("ctl_newchat", "new_chat", (20, 50, 140, 30))
    sc.observe("New chat", 48, 54, 120, 22)
    # the search field
    sc.put(surface("sf", 20, 92, 366, 40, bg="white", radius=10, border=1))
    sc.put(icon("i_search", "search", 34, 104, 16, 16, color="muted"))
    if search_text:
        sc.put(text("t_search", search_text, 58, 102, 240, 22, color="ink"))
    else:
        sc.put(text("t_search_ph", "Search chats", 58, 102, 240, 22, color="muted"))
    if clear:
        sc.put(icon("i_clear", "close", 356, 104, 14, 14, color="muted"))
        sc.control("ctl_search_clear", "search.clear", (348, 98, 26, 26))
    sc.control("ctl_search", "search.focus", (20, 92, 366, 40))
    sc.observe(search_text or "Search chats", 58, 102, 240, 22)


def segmented(sc, *, left_selected=True):
    sc.put(surface("seg", 20, 148, 214, 34, bg="chip", radius=9))
    sel_x = 24 if left_selected else 129
    sc.put(surface("seg_sel", sel_x, 152, 105, 26, bg="white", radius=7))
    sc.put(text("t_seg_l", "By workspace", 24, 156, 105, 20, weight=500,
                color="ink" if left_selected else "muted", size=13))
    sc.put(text("t_seg_r", "All", 129, 156, 105, 20, weight=500,
                color="ink" if not left_selected else "muted", size=13))
    sc.control("ctl_seg_ws", "sidebar.mode.grouped", (24, 152, 105, 26))
    sc.control("ctl_seg_all", "sidebar.mode.flat", (129, 152, 105, 26))
    sc.put(icon("i_sort", "chev-down", 380, 156, 14, 14, color="ink"))
    sc.put(text("t_sort", "Recent", 330, 156, 48, 20, color="ink", size=13))
    sc.observe("By workspace", 24, 156, 105, 20)
    sc.observe("Recent", 330, 156, 48, 20)


def row(sc, i, y, title, when, *, selected=False, indent=56):
    iid = f"r{i:02d}"
    if selected:
        sc.put(surface(f"{iid}_sel", 16, y - 6, 374, 36, bg="sel", radius=8))
    sc.put(text(f"{iid}_t", title, indent, y, 250, 20, weight=400))
    sc.put(text(f"{iid}_w", when, 300, y, 84, 20, color="muted", size=12))
    sc.control(f"ctl_row{i}", "session.open", (16, y - 6, 374, 36))
    sc.observe(title, indent, y, 250, 20)
    sc.observe(when, 300, y, 84, 20)


def group_head(sc, y, name, *, expanded=True, count=None, menu=False):
    sc.put(icon(f"i_chev_{name.replace(' ', '')}", "chev-down" if expanded else "chev-right",
                24, y + 2, 14, 14))
    sc.put(text(f"t_grp_{name.replace(' ', '')}", name, 46, y, 180, 22, weight=600, size=15))
    sc.control(f"ctl_grp_{name.replace(' ', '')}", "workspace.toggle",
               (16, y - 4, 250, 30))
    sc.observe(name, 46, y, 180, 22)
    x = 320
    if count is not None:
        sc.put(surface("grp_badge", 320, y, 30, 22, bg="chip", radius=11))
        sc.put(text("grp_badge_t", str(count), 320, y + 2, 30, 18, color="ink",
                    size=12))
        sc.observe(str(count), 320, y, 30, 22)
    if menu:
        sc.put(surface("grp_menu_btn", 356, y, 30, 22, bg="chip", radius=11))
        sc.put(icon("i_dots", "dots", 362, y + 4, 18, 14, color="ink"))
        sc.control("ctl_grp_menu", "workspace.menu", (356, y - 4, 34, 30))
        sc.observe("menu", 356, y, 30, 22)


def menu_popup(sc):
    sc.put(surface("menu", 322, 246, 150, 104, bg="white", radius=10, border=1))
    for k, (label, ev) in enumerate([("New chat here", "workspace.new_chat_here"),
                                     ("Rename", "workspace.rename"),
                                     ("Remove from sidebar", "workspace.remove")]):
        y = 258 + k * 30
        sc.put(text(f"menu_t{k}", label, 336, y, 128, 20, size=13))
        sc.control(f"ctl_menu{k}", ev, (326, y - 6, 142, 28))
        sc.observe(label, 336, y, 128, 20)


def add_workspace(sc, y=726):
    sc.put(icon("i_plus", "plus", 24, y, 16, 16))
    sc.put(text("t_addws", "Add workspace", 48, y - 2, 160, 20, weight=500))
    sc.control("ctl_addws", "workspace.add", (16, y - 8, 220, 32))
    sc.observe("Add workspace", 48, y - 2, 160, 20)


# ----------------------------------------------------------------- screens
def build_01(sc):
    header(sc)
    segmented(sc, left_selected=True)
    y = 214
    group_head(sc, y, "octos", expanded=True)
    y += 36
    for k, (t, w) in enumerate(ROWS["octos"]):
        row(sc, k, y, t, w, selected=(k == 0))
        y += 40
    y += 18
    group_head(sc, y, "octoscode-app", expanded=True)
    y += 36
    for k, (t, w) in enumerate(ROWS["octoscode-app"]):
        row(sc, len(ROWS["octos"]) + k, y, t, w)
        y += 40
    add_workspace(sc)


def build_02(sc):
    header(sc)
    segmented(sc, left_selected=False)
    y = 214
    for k, (label, when, glyph) in enumerate(STATUSES):
        color = {"blue-dot": "blue", "amber-dot": "amber", "check": "muted",
                 "red-dot": "red", "hollow": "muted"}[glyph]
        sc.put(icon(f"i_st{k}", {"blue-dot": "dot", "amber-dot": "dot",
                                 "check": "check", "red-dot": "dot",
                                 "hollow": "hollow"}[glyph],
                    24, y + 1, 16, 16, color=color))
        sc.put(text(f"st_t{k}", label, 48, y, 240, 20, weight=500))
        sc.put(text(f"st_w{k}", when, 300, y, 84, 20, color="muted", size=12))
        sc.control(f"ctl_strow{k}", "session.open", (16, y - 6, 374, 36))
        sc.observe(label, 48, y, 240, 20)
        sc.observe(when, 300, y, 84, 20)
        y += 42
    add_workspace(sc)


def build_03(sc):
    header(sc, search_text="hydrate", clear=True)
    segmented(sc, left_selected=True)
    y = 214
    group_head(sc, y, "octos", expanded=True)
    y += 36
    # the matching row: "hydrate" highlighted inside the title
    sc.put(surface("r00_sel", 16, y - 6, 374, 36, bg="sel", radius=8))
    sc.put(text("m1", "Why is ", 56, y, 62, 20))
    sc.put(surface("hl", 118, y + 1, 62, 20, bg="hl", radius=3))
    sc.put(text("m2", "hydrate", 120, y, 58, 20))
    sc.put(text("m3", " slow?", 178, y, 56, 20))
    sc.put(text("r00_w", "Yesterday", 300, y, 84, 20, color="muted", size=12))
    sc.control("ctl_row0", "session.open", (16, y - 6, 374, 36))
    sc.observe("Why is hydrate slow?", 56, y, 240, 20)
    y += 56
    group_head(sc, y, "octoscode-app", expanded=True)
    y += 34
    sc.put(text("empty1", 'No chats in octoscode-app match "hydrate"',
                56, y, 300, 20, color="muted"))
    sc.observe('No chats in octoscode-app match "hydrate"', 56, y, 300, 20)
    y += 30
    sc.put(text("clear1", "Clear search", 56, y, 110, 20, color="blue"))
    sc.control("ctl_clear_search", "search.clear", (50, y - 6, 122, 30))
    sc.observe("Clear search", 56, y, 110, 20)
    add_workspace(sc)


def build_04(sc):
    header(sc)
    segmented(sc, left_selected=True)
    y = 214
    group_head(sc, y, "octos", expanded=False, count=3, menu=True)
    menu_popup(sc)
    y += 52
    group_head(sc, y, "octoscode-app", expanded=True)
    y += 36
    for k, (t, w) in enumerate(ROWS["octoscode-app"]):
        row(sc, k, y, t, w)
        y += 40
    add_workspace(sc)


def build_05(sc):
    # the dimmed conversation strip behind, right of the drawer
    sc.put(surface("behind", 0, 0, 406, 776, bg="panel", radius=0))
    sc.put(surface("behind_col", 357, 0, 49, 776, bg="white", radius=0))
    sc.put(surface("dim", 0, 0, 406, 776, bg="dim", radius=0))
    # the drawer slides over the left 88%
    sc.put(surface("drawer", 0, 0, 357, 776, bg="white", radius=0))
    sc.put(icon("i_close", "close", 322, 18, 16, 16))
    sc.control("ctl_drawer_close", "drawer.close", (314, 10, 32, 32))
    sc.observe("close", 322, 18, 16, 16)
    header(sc)
    segmented(sc, left_selected=True)
    y = 214
    group_head(sc, y, "octos", expanded=True)
    y += 36
    for k, (t, w) in enumerate(ROWS["octos"]):
        row(sc, k, y, t, w, selected=(k == 0), indent=50)
        y += 40
    y += 18
    group_head(sc, y, "octoscode-app", expanded=True)
    y += 36
    for k, (t, w) in enumerate(ROWS["octoscode-app"]):
        row(sc, len(ROWS["octos"]) + k, y, t, w, indent=50)
        y += 40
    add_workspace(sc)


BUILD = {1: build_01, 2: build_02, 3: build_03, 4: build_04, 5: build_05}


def build(num):
    sc = Scene(num)
    BUILD[num](sc)
    doc = {"schema_version": 1, "id": f"phase4n2-{num:02d}", "app": "octoscode",
           "number": num, "title": TITLES[num],
           "structure": "Native component reconstructed from the approved board-2 atlas",
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                       "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
           "content_source": ("Approved stage-a phase4-new2 atlas; text/bounds measured by "
                              "the lane model's vision on the measured hairline grid crops"),
           "graphics": {},
           "tree": {"id": "page", "type": "surface", "x": 0, "y": 0, "w": 406, "h": 776,
                    "bg": "#FFFFFF", "radius": 0, "kids": sc.kids}}
    return doc, sc


def main():
    only = [int(x) for x in sys.argv[1:]] or [1, 2, 3, 4, 5]
    for n in only:
        ICON_REG.clear()
        doc, sc = build(n)
        d = ROOT / "cards" / f"phase4n2-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        obs = {"width": 406, "height": 776, "observations": sc.obs,
               "source": "lane-vision on the measured atlas crops (no external OCR for board 2)"}
        (d / "observations.json").write_text(json.dumps(obs, indent=2, ensure_ascii=False) + "\n")
        for iid, (name, color) in ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(svg(name, hexpal().get(color, "#1D1D1F")))
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": sc.controls, "source": SOURCE[n]}, indent=2) + "\n")
        print(f"wrote phase4n2-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(ICON_REG)} icons, {len(sc.controls)} controls")


if __name__ == "__main__":
    main()
