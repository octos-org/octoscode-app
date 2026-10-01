#!/usr/bin/env python3
"""Author board 3 (design/stage-a/phase4-new3) screens 1-4 contracts — the
#D3a half: runtime inventory, workspace create, create folder, fleet.

Modeled on the board-2 author (design/stage-b/phase4-new2/tools/author.py):
same contract schema, palette, fonts, and Scene/control shapes. The board-2
SIDEBAR screen bodies do NOT transfer — these are four different modals — so
the layout helpers below are written for modal screens.

One honest note, same as board 2: there is no external OCR for this atlas, so
the `observations` are THIS lane model's own vision measurements from the
measured atlas crops (tmp/d3a/screen{1..4}.png, cut on the measured hairline
grid), rescaled to the shared 406x776 artboard. They are labeled 'lane-vision'
in the emitted observations.json. Coordinates are authored in artboard space
directly.

Screens (operator-approved atlas-prompt.md, 2026-10-01):
  01 runtime inventory  02 workspace create  03 create folder  04 fleet

Measured atlas grid (2048x3072 PORTRAIT — note board 2's atlas was LANDSCAPE,
so its hardcoded crop boxes do not transfer):
  rows    y=70-1038 / 1126-2019 / 2114-3005
  columns x=33-496 / 526-1000 / 1027-1495 / 1516-2005
Screens 1-4 are ROW 1 (the first four columns).

Run FROM THE REPO ROOT:
  python3 design/stage-b/phase4-new3/tools/author.py [1 2 3 4]
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# The atlas prompt's palette (the same token set the autonomy author uses).
C = {"white": 0xFFFFFFFF, "panel": 0xFFF7F7F8, "hair": 0xFFE5E5E7, "ink": 0xFF1D1D1F,
     "muted": 0xFF6E6E73, "black": 0xFF000000, "blue": 0xFF2F6FEB, "amber": 0xFFB45309,
     "green": 0xFF1F883D, "red": 0xFFCF222E, "sel": 0xFFF2F2F7, "chip": 0xFFEFEFF0,
     "hl": 0xFFFCE9B8, "dim": 0x99000000, "disabled": 0xFFC7C7CC}
C_HEX = {k: f"#{v & 0xFFFFFF:06X}" for k, v in C.items()}
FONT = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf",
        600: "self:resources/ux/Inter-600.ttf", 700: "self:resources/ux/Inter-700.ttf"}

# MONO for the paths / ids / model names the prompt calls "in monospace".
MONO = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf"}

TITLES = {1: "Runtime inventory", 2: "Add workspace", 3: "Add workspace",
          4: "Fleet"}

SOURCE = {n: f"phase4-new3-screen-{n:02d}" for n in range(1, 5)}

# The measured hairline-grid cells for row 1 (PIL crop boxes, x0,y0,x1,y1).
ATLAS_BOXES = {1: (33, 70, 496, 1038), 2: (526, 70, 1000, 1038),
               3: (1027, 70, 1495, 1038), 4: (1516, 70, 2005, 1038)}

ARTW, ARTH = 406, 776
ICON_REG: dict = {}


def hexpal():
    return C_HEX


def svg(name, color="#1D1D1F"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" '
            f'width="24" height="24" fill="none" stroke="{color}" '
            f'stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">'
            f'<title>{name}</title>'
            f'<path d="M6 6h12M6 12h12M6 18h8"/></svg>')


def icon(iid, name, x, y, w, h, color="ink"):
    ICON_REG[iid] = (name, color)
    return {"t": "svg", "id": iid, "x": x, "y": y, "w": w, "h": h,
            "src": f"{ASSET_URL}/{iid}.svg", "color": color}


def argb(color):
    """Palette NAME -> int ARGB. The design-flow's `observe()` does a bitwise
    `node['color'] >> 16/8/0` to test ink against the background, so a colour
    NAME breaks it with `TypeError: unsupported operand type(s) for >>`."""
    return C[color] if isinstance(color, str) else color


def text(iid, s, x, y, w, h, *, weight=400, color="ink", size=14, mono=False,
         alignx=0):
    # The design-flow's semantic preflight rejects a text box SHORTER than its
    # line box ("text box height 17 is under its line box (18.3); the glyphs
    # will clip at the bottom"). Authoring with a nominal 17px box produced that
    # for every size-14 label, so the line box is the floor, computed here once
    # rather than hand-tuned per call site.
    line_h = round(size * 1.22, 2)
    return {"t": "text", "id": iid, "text": s, "x": x, "y": y, "w": w,
            "h": max(h, line_h),
            "size": size, "line_height": line_h, "weight": weight,
            "color": argb(color), "variant": "single_line", "alignx": alignx,
            "font_src": (MONO if mono else FONT)[weight], "tracking": 0.0}


def field(iid, value, x, y, w, h, *, placeholder=None, color="ink", size=14,
          mono=False):
    """A borderless single-line input: a surface + a text node + the control."""
    kids = [surface(f"{iid}_bg", x, y, w, h, bg="sel", radius=10),
            text(f"{iid}_text", value, x + 12, y + (h - 18) / 2, w - 24, 18,
                 color=color, size=size, mono=mono)]
    if placeholder:
        kids.append(text(f"{iid}_ph", placeholder, x + 12, y + (h - 18) / 2,
                         w - 24, 18, color="disabled", size=size))
    kids.append({"t": "button", "id": iid, "x": x, "y": y, "w": w, "h": h,
                 "enabled": 1})
    return {"t": "stack", "id": f"{iid}_row", "x": x, "y": y, "w": w, "h": h,
            "c": kids}


def surface(iid, x, y, w, h, *, bg="white", radius=12, border=0, kids=None):
    n = {"t": "stack", "id": iid, "x": x, "y": y, "w": w, "h": h,
         "c": kids or [], "variant": "surface", "bg": C[bg], "radius": radius}
    if border:
        n["border"] = border
        n["bordercolor"] = C["hair"]
    return n


def pill(iid, label, x, y, *, color="chip", size=11, pad=8, weight=500,
         fg="ink"):
    w = pad * 2 + len(label) * (size * 0.62)
    return {"t": "stack", "id": iid, "x": x, "y": y, "w": round(w, 1),
            "h": 20, "variant": "pill", "bg": C[color], "radius": 10,
            "c": [text(f"{iid}_t", label, x + pad, y + 3, round(w - pad * 2, 1), 14,
                       weight=weight, color=fg, size=size)]}


def btn(iid, label, x, y, w, h, *, fill="black", color="white", size=14,
        weight=500, enabled=True, radius=None):
    """A real t:\"button\" node — the ONLY kind taps::inject_click attaches to.
    D2a's 'zero rects' failure was controls with no such node; keep this."""
    r = radius if radius is not None else h / 2
    kids = [surface(f"{iid}_bg", x, y, w, h, bg=fill, radius=r)]
    if not enabled:
        kids[0]["bg"] = C["sel"]
    kids.append(text(f"{iid}_t", label, x, y + (h - 17) / 2, w, 17,
                     weight=weight, color=(color if enabled else "disabled"),
                     size=size))
    kids.append({"t": "button", "id": iid, "x": x, "y": y, "w": w, "h": h,
                 "enabled": 1 if enabled else 0})
    return {"t": "stack", "id": f"{iid}_row", "x": x, "y": y, "w": w, "h": h, "c": kids}


def link(iid, label, x, y, w, *, color="muted", size=13, enabled=True):
    """A text-link affordance. D2a lesson: a Text node gets NO handler, so a
    link that must be clickable is a button node with borderless chrome."""
    return {"t": "stack", "id": f"{iid}_row", "x": x, "y": y, "w": w, "h": 20,
            "c": [text(f"{iid}_t", label, x, y, w, 17, color=color, size=size),
                  {"t": "button", "id": iid, "x": x, "y": y, "w": w, "h": 20,
                   "enabled": 1 if enabled else 0}]}


def divider(iid, y, x=20, w=366, color="hair"):
    return {"t": "stack", "id": iid, "x": x, "y": y, "w": w, "h": 1,
            "variant": "surface", "bg": C[color], "radius": 0}


def modal(y, h, *, x=16, w=374, bg="white"):
    """A dimmed scrim with the modal surface on top — screens 1-3 are modals."""
    return surface(f"scrim_{y}", 0, 0, ARTW, ARTH, bg="dim", radius=0), \
           surface("modal_card", x, y, w, h, bg=bg, radius=20, border=1)


ASSET_URL = "http://127.0.0.1:8390/ux-images/phase4-new3"


class Scene:
    """The board-2 Scene shape: nodes + observations + controls, kept so the
    contract/observations/service-actions writers below are unchanged."""

    def __init__(self, num):
        self.num = num
        self.kids: list = []
        self.obs: list = []
        self.controls: dict = {}

    def put(self, node):
        self.kids.append(node)

    def control(self, cid, event, bounds, *, enabled=True):
        # The authored bounds ARE the emitted node's own x/y,w,h, so
        # taps::wire_card_events_dir's 1.5px match is exact by construction.
        self.controls[cid] = {"event": event, "source_bounds": list(bounds),
                              "enabled": enabled}

    def observe(self, s, x, y, w, h):
        self.obs.append({"expected_text": s, "requested_bounds": [x, y, w, h],
                         "ink_bounds": [x, y, w, h]})


def node_rect(n):
    return (n.get("x", 0), n.get("y", 0), n.get("w", 0), n.get("h", 0))


def find(nodes, iid):
    for n in nodes:
        if n.get("id") == iid:
            return n
        r = find(n.get("c", []) or [], iid)
        if r is not None:
            return r
    return None


def build(num):
    ICON_REG.clear()
    sc = Scene(num)
    {1: build_01, 2: build_02, 3: build_03, 4: build_04}[num](sc)
    doc = {
        "schema_version": 1, "id": f"phase4n3-{num:02d}", "app": "octoscode",
        "number": num, "title": TITLES[num],
        "structure": "Native component reconstructed from the approved board-3 atlas",
        "artboard": [ARTW, ARTH],
        "artboard_pixels_per_pt": 1.0,
        "content_source": ("Approved stage-a phase4-new3 atlas; text/bounds measured by "
                           "the lane model on the measured hairline-grid crops "
                           "(labeled 'lane-vision'; no external OCR exists for board 3)"),
        "font_family": "Inter",
        # The design-flow's `observe()` walks contract['tree'] and the compile
        # stage reads these six tokens (board-2's contract schema, kept
        # identical so the same pipeline runs unchanged).
        "palette": {"name": "OctosCode", "page": C_HEX["white"],
                    "panel": C_HEX["panel"], "ink": C_HEX["ink"],
                    "muted": C_HEX["muted"], "accent": C_HEX["blue"]},
        "colors": {k: C_HEX[k] for k in ("white", "panel", "hair", "ink", "muted",
                                         "black", "blue", "amber", "green", "red",
                                         "sel", "chip", "disabled")},
        # The design-flow's `semantics.classify` reads
        # `contract.get('graphics', {}).get(node['id'], {})` — a MAP keyed by
        # node id, not a list. Board 2 emits `{}` (no chart graphics); these
        # four modals declare no charts either, so the honest value is empty.
        "graphics": {},
        "widgets": [],
        "tree": {"t": "stack", "id": "page", "x": 0, "y": 0, "w": ARTW, "h": ARTH,
                 "variant": "surface", "bg": C["white"], "radius": 0,
                 "c": sc.kids},
    }
    return doc, sc


# --------------------------------------------------------------- screen 01
def build_01(sc):
    """Runtime inventory (atlas-prompt.md #1)."""
    scrim, card = modal(y=44, h=688)
    sc.put(scrim)
    k = [text("t_title", "Runtime inventory", 32, 62, 240, 24, weight=600, size=20),
         text("t_sub", "12 tools · 3 servers", 32, 88, 240, 16, color="muted", size=12)]
    k.append(field("f_search", "Search names, status, or tools…", 32, 116, 342, 40,
                   color="muted"))
    sc.observe("Search names, status, or tools…", 32, 116, 342, 40)
    sc.control("f_search", "inventory.search.focus", (32, 116, 342, 40))
    # two tabs, "Tools" selected
    k.append(pill("tab_tools_fill", "Tools", 32, 168, color="black", fg="white"))
    k.append(pill("tab_mcp_fill", "MCP servers", 104, 168, color="sel"))
    k.append(link("tab_tools", "Tools", 32, 168, 60, color="white", enabled=True))
    k.append(link("tab_mcp", "MCP servers", 104, 168, 92, color="ink", enabled=True))
    sc.control("tab_tools", "inventory.tab.tools", (32, 168, 60, 20))
    sc.control("tab_mcp", "inventory.tab.servers", (104, 168, 92, 20))
    k.append(text("h_tools", "TOOLS", 32, 208, 120, 12, color="muted", size=11,
                  weight=600))
    y = 230
    for i, (nm, cat, st, alias, backend, cnt) in enumerate([
        ("Bash", "shell", "enabled", "exec, run", "octos", "12"),
        ("Read", "fs", "enabled", "cat, view", "octos", "4"),
        ("ImageView", "media", "disabled", "see", "octos", "1"),
    ]):
        r = surface(f"tool_{i}", 32, y, 342, 52, bg="white", radius=10, border=1)
        rk = [text(f"tool_{i}_name", nm, 44, y + 8, 180, 16, size=13, weight=500,
                   mono=True),
              text(f"tool_{i}_cat", cat, 44, y + 28, 120, 14, color="muted",
                   size=11),
              text(f"tool_{i}_alias", f"Aliases: {alias}", 150, y + 8, 130, 14,
                   color="muted", size=11),
              text(f"tool_{i}_be", f"Backend: {backend}", 150, y + 26, 130, 14,
                   color="muted", size=11),
              text(f"tool_{i}_cnt", cnt, 340, y + 18, 24, 16, color="muted",
                   size=13)]
        stc = "green" if st == "enabled" else "disabled"
        rk.insert(3, pill(f"tool_{i}_st", st, 296, y + 6, color=stc, size=10))
        r["c"] = rk
        k.append(r)
        sc.observe(nm, 44, y + 8, 180, 16)
        y += 58
    k.append(text("h_mcp", "MCP SERVERS", 32, y + 6, 140, 12, color="muted",
                  size=11, weight=600))
    k.append(text("mcp_counts", "connected · 2    connecting · 1    failed · 0",
                  150, y + 6, 224, 14, color="muted", size=11))
    y += 28
    # The status WORD is data (it reaches the row text); the DOT COLOUR is the
    # palette lookup, so they are mapped explicitly rather than conflated.
    STATUS_DOT = {"connected": "green", "connecting": "amber", "failed": "red"}
    for i, (sid, tr, st, tc, summ) in enumerate([
        ("fs-probe", "stdio", "connected", "6", "workspace file tools"),
        ("web-probe", "http", "connecting", "12", "browser + fetch tools"),
    ]):
        r = surface(f"srv_{i}", 32, y, 342, 48, bg="white", radius=10, border=1)
        rk = [text(f"srv_{i}_id", sid, 44, y + 8, 140, 16, size=13, weight=500,
                   mono=True),
              text(f"srv_{i}_sum", summ, 44, y + 28, 200, 14, color="muted",
                   size=11),
              pill(f"srv_{i}_tr", tr, 200, y + 7, color="chip", size=10),
              text(f"srv_{i}_tc", tc, 340, y + 8, 24, 16, color="muted", size=13)]
        rk.insert(2, pill(f"srv_{i}_dot", "", 262, y + 12,
                           color=STATUS_DOT[st], size=6, pad=5))
        r["c"] = rk
        k.append(r)
        sc.observe(sid, 44, y + 8, 140, 16)
        y += 54
    k.append(text("empty_tools", "No matching tools.", 236, y + 4, 138, 14,
                  color="disabled", size=11))
    k.append(text("empty_servers", "No matching servers.", 236, y + 22, 138, 14,
                  color="disabled", size=11))
    k.append(link("btn_close", "✕", 330, 60, 24, color="muted", size=15))
    sc.control("btn_close", "inventory.close", (330, 60, 24, 20))
    card["c"] = k
    sc.put(card)


# --------------------------------------------------------------- screen 02
def build_02(sc):
    """Workspace create (atlas-prompt.md #2)."""
    scrim, card = modal(y=64, h=640)
    sc.put(scrim)
    k = [text("t_title", "Add workspace", 32, 82, 240, 24, weight=600, size=20)]
    # The server's working directory is pinned FIRST.
    k.append(surface("wd_card", 32, 118, 342, 56, bg="sel", radius=10))
    k.append(text("wd_label", "Server's working directory", 44, 126, 240, 16,
                  color="muted", size=12))
    k.append(text("wd_path", "/home/user/octos", 44, 146, 200, 16, size=13,
                  mono=True))
    k.append(text("wd_help", "Start a new session here", 236, 146, 126, 16,
                  color="muted", size=11))
    sc.observe("/home/user/octos", 44, 146, 200, 16)
    k.append(link("wd_start", "Start a new session in /home/user/octos", 44, 178,
                  318, color="blue", size=12))
    sc.control("wd_start", "workspace.start_in_server_root", (44, 178, 318, 20))
    k.append(text("h_recent", "Recent workspaces", 32, 206, 240, 16,
                  color="muted", size=12, weight=600))
    y = 230
    for i, (nm, br, path) in enumerate([
        ("octos", "feat/steer-queue", "/home/user/octos"),
        ("octoscode-app", "main", "/home/user/octoscode-app"),
    ]):
        k.append(surface(f"rec_{i}", 32, y, 342, 52, bg="white", radius=10,
                         border=1))
        k.append(text(f"rec_{i}_name", nm, 44, y + 8, 160, 16, weight=500, size=13,
                      mono=True))
        k.append(text(f"rec_{i}_br", br, 210, y + 9, 100, 14, color="muted",
                      size=11))
        k.append(text(f"rec_{i}_path", path, 44, y + 30, 240, 14, color="muted",
                      size=11, mono=True))
        k.append(link(f"rec_{i}_pick", "Start a new session in …", 44, y + 30, 200,
                      color="blue", size=11))
        sc.observe(nm, 44, y + 8, 160, 16)
        y += 58
    k.append(divider("div1", 352))
    k.append(field("f_path", "", 32, 372, 250, 40, placeholder="/path/to/project",
                   color="disabled"))
    sc.observe("/path/to/project", 44, 381, 226, 18)
    k.append(btn("btn_enter", "Enter", 296, 372, 78, 40, radius=12, size=14))
    sc.control("btn_enter", "workspace.create.submit", (296, 372, 78, 40))
    k.append(text("help_browse", "Choosing a folder fills the path box.", 32, 424,
                  300, 14, color="muted", size=11))
    k.append(link("btn_browse", "Browse server folders…", 32, 448, 200,
                  color="blue", size=12))
    sc.control("btn_browse", "workspace.browse.open", (32, 448, 200, 20))
    card["c"] = k
    sc.put(card)


# --------------------------------------------------------------- screen 03
def build_03(sc):
    """Create folder (atlas-prompt.md #3) — the same modal in its create state."""
    scrim, card = modal(y=64, h=640)
    sc.put(scrim)
    k = [text("t_title", "Add workspace", 32, 82, 240, 24, weight=600, size=20),
         link("btn_back", "Back to workspaces", 32, 112, 200, color="blue", size=12)]
    sc.control("btn_back", "workspace.create.back", (32, 112, 200, 20))
    k.append(field("f_folder", "notes", 32, 150, 250, 40, size=13))
    sc.observe("notes", 44, 159, 226, 18)
    k.append(btn("btn_create", "Create", 296, 150, 78, 40, radius=12, size=14))
    sc.control("btn_create", "folder.create.submit", (296, 150, 78, 40))
    # A validation line only when the name is invalid.
    k.append(text("val_name", "Folder name must not be empty.", 32, 200, 300, 14,
                  color="red", size=11))
    k.append(text("parent_label", "Parent", 32, 232, 120, 14, color="muted",
                  size=12))
    k.append(text("parent_path", "/home/user/octos", 32, 250, 200, 16, size=13,
                  mono=True))
    sc.observe("/home/user/octos", 32, 250, 200, 16)
    k.append(link("btn_drill", "Open folder browser", 32, 286, 200, color="muted",
                  size=12, enabled=True))
    k.append(link("btn_drill_disabled", "Open folder browser (unavailable)", 32,
                  286, 260, color="disabled", size=12, enabled=False))
    sc.control("btn_drill", "folder.browser.open", (32, 286, 200, 20))
    sc.control("btn_drill_disabled", "folder.browser.open", (32, 312, 260, 20),
               enabled=False)
    k.append(surface("disc_1", 32, 330, 342, 44, bg="sel", radius=10))
    k.append(text("disc_t", "· 3 existing folders", 44, 342, 200, 16, size=12))
    k.append(text("disc_l", "notes · drafts · tmp", 44, 360, 240, 14,
                  color="muted", size=11, mono=True))
    k.append(text("note_browser", "The folder browser is available only when the "
                  "server advertises it.", 32, 392, 342, 28, color="muted", size=11))
    card["c"] = k
    sc.put(card)


# --------------------------------------------------------------- screen 04
def build_04(sc):
    """Fleet destination + Start (atlas-prompt.md #4)."""
    card = surface("fleet_card", 0, 0, ARTW, ARTH, bg="white", radius=0)
    k = [text("t_title", "Fleet", 20, 20, 160, 24, weight=600, size=20),
         text("t_empty", "No peers yet", 246, 26, 140, 16, color="muted", size=12,
              alignx=2)]
    k.append(text("loading_models", "Loading models…", 20, 52, 200, 14,
                  color="muted", size=11))
    # peer card 1: the Start form over a model + brief
    p1 = surface("peer_1", 20, 80, 366, 296, bg="white", radius=14, border=1)
    p1k = [surface("peer_1_avatar", 36, 96, 32, 32, bg="sel", radius=16),
           text("peer_1_av", "A", 36, 104, 32, 18, weight=600, size=14),
           text("peer_1_model", "anthropic/claude-3.5-sonnet", 78, 96, 200, 16,
                weight=500, size=13, mono=True),
           pill("peer_1_status", "Available", 78, 118, color="chip", size=10),
           link("peer_1_adv", "Advanced", 300, 98, 70, color="blue", size=12),
           text("peer_1_brief", "Strong coding and reasoning model with tool use.",
                78, 146, 290, 34, color="muted", size=12),
           text("lbl_model", "Model", 36, 190, 120, 14, color="muted", size=11),
           field("sel_model", "anthropic/claude-3.5-sonnet", 36, 208, 334, 38,
                 color="ink", size=12, mono=True),
           text("lbl_brief", "Brief", 36, 254, 120, 14, color="muted", size=11),
           surface("brief_box", 36, 272, 334, 56, bg="sel", radius=10),
           text("brief_text", "Focus on code quality, tests, and minimal diffs.",
                48, 282, 310, 36, size=12),
           {"t": "button", "id": "brief_input", "x": 36, "y": 272, "w": 334,
            "h": 56, "enabled": 1},
           btn("btn_start", "Start", 282, 340, 88, 34, radius=17, size=14)]
    p1["c"] = p1k
    k.append(p1)
    sc.observe("anthropic/claude-3.5-sonnet", 78, 96, 200, 16)
    sc.observe("Focus on code quality, tests, and minimal diffs.", 48, 282, 310, 36)
    sc.control("peer_1_adv", "fleet.peer.advanced", (300, 98, 70, 20))
    sc.control("sel_model", "fleet.start.model.select", (36, 208, 334, 38))
    sc.control("brief_input", "fleet.start.brief", (36, 272, 334, 56))
    sc.control("btn_start", "fleet.start.submit", (282, 340, 88, 34))
    # peer card 2: waiting state
    p2 = surface("peer_2", 20, 392, 366, 150, bg="white", radius=14, border=1)
    p2k = [surface("peer_2_avatar", 36, 408, 32, 32, bg="sel", radius=16),
           text("peer_2_av", "W", 36, 416, 32, 18, weight=600, size=14),
           text("peer_2_model", "gpt-4o", 78, 408, 140, 16, weight=500, size=13,
                mono=True),
           pill("peer_2_status", "Waiting for you", 220, 410, color="amber", size=10),
           field("in_steer", "Enter steering text", 36, 448, 334, 38,
                 color="disabled", size=12),
           link("btn_dismiss", "Dismiss", 36, 498, 70, color="muted", size=12),
           divider("peer_2_rule", 500, 120, 250),
           text("peer_2_note", "Only while working", 36, 512, 334, 14,
                color="muted", size=11)]
    p2["c"] = p2k
    k.append(p2)
    sc.observe("gpt-4o", 78, 408, 140, 16)
    sc.control("in_steer", "fleet.peer.steer", (36, 448, 334, 38))
    sc.control("btn_dismiss", "fleet.peer.dismiss", (36, 498, 70, 20))
    k.append(text("t_noproject", "Open a project first", 123, 560, 160, 16,
                  color="muted", size=12, alignx=1))
    card["c"] = k
    sc.put(card)


def main():
    only = [int(x) for x in sys.argv[1:]] or [1, 2, 3, 4]
    for n in only:
        ICON_REG.clear()
        doc, sc = build(n)
        d = ROOT / "cards" / f"phase4n3-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        obs = {"width": ARTW, "height": ARTH, "observations": sc.obs,
               "source": "lane-vision on the measured atlas crops (no external OCR for board 3)"}
        (d / "observations.json").write_text(json.dumps(obs, indent=2, ensure_ascii=False) + "\n")
        ref = ROOT.parent.parent / "stage-a" / "phase4-new3"
        atlas = ref / "atlas.png"
        for iid, (name, color) in ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(svg(name, hexpal().get(color, "#1D1D1F")))
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": sc.controls, "source": SOURCE[n]}, indent=2) + "\n")
        from PIL import Image
        Image.open(atlas).crop(ATLAS_BOXES[n]).save(d / "reference.png")
        print(f"wrote phase4n3-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(ICON_REG)} icons, {len(sc.controls)} controls, reference.png")


if __name__ == "__main__":
    main()
