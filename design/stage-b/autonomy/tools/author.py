#!/usr/bin/env python3
"""Author the autonomy board-3 screens 7-12 contracts, modeled on board 1's
tools/author_v2.py (same helpers, palette, fonts, index-driven OCR composition).

Screens: 07 Tasks, 08 Resume, 09 Attachments, 10 Side question,
         11 Dark conversation, 12 Dark settings.

Dark screens (11/12) reuse the SAME components with a dark token set
(per the #28b entry: dark mode = a dark token set, not new components).

Run: python3 design/stage-b/autonomy/tools/author.py  [writes contract.json + assets/*.svg + service-actions.json]
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OCR = ROOT / "ocr"

C = {"white": 0xFFFFFFFF, "panel": 0xFFF7F7F8, "hair": 0xFFE5E5E7, "ink": 0xFF1D1D1F,
     "muted": 0xFF6E6E73, "black": 0xFF000000, "blue": 0xFF2F6FEB, "green": 0xFF1F883D,
     "greenbg": 0xFFE6F4EA, "red": 0xFFCF222E, "redbg": 0xFFFDECEC, "sel": 0xFFF2F2F7,
     "mono": 0xFFF6F6F7, "box": 0xFFF4F4F5, "page": 0xFFFFFFFF}
C_HEX = {k: f"#{v & 0xFFFFFF:06X}" for k, v in C.items()}
FONT = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf",
        600: "self:resources/ux/Inter-600.ttf", 700: "self:resources/ux/Inter-700.ttf"}
MONO = "self:resources/ux/LiberationMono-Regular.ttf"

# Dark token set for screens 11/12 — same components, dark values.
CD = {"page": 0xFF1C1C1E, "panel": 0xFF2C2C2E, "hair": 0xFF3A3A3C, "ink": 0xFFF2F2F7,
      "muted": 0xFF98989D, "black": 0xFF3A3A3C, "box": 0xFF242426,
      "white": 0xFF2C2C2E, "sel": 0xFF3A3A3C, "mono": 0xFF242426}
CD_HEX = {k: f"#{v & 0xFFFFFF:06X}" for k, v in CD.items()}

# Active palette — switched per scene by build() before the builder runs.
_ACTIVE_C = C
_ACTIVE_HEX = C_HEX


def pal():
    return _ACTIVE_C


def hexpal():
    return _ACTIVE_HEX

ICONS = {
    "plus": '<path d="M12 5v14"/><path d="M5 12h14"/>',
    "send": '<path d="M5 12h13"/><path d="M12 6l6 6-6 6"/>',
    "mic": '<rect x="9" y="2.6" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0"/><path d="M12 17.5V21"/>',
    "chevron": '<path d="M9 6l6 6-6 6"/>',
    "chevron_down": '<path d="M6 9l6 6 6-6"/>',
    "spinner": '<path d="M12 3a9 9 0 1 0 9 9"/>',
    "check": '<path d="M5 12.5l4.5 4.5L19 7"/>',
    "x": '<path d="M6 6l12 12"/><path d="M18 6L6 18"/>',
    "doc": '<path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4"/>',
    "image": '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="M5 17l5-4 4 3 3-2 2 3"/>',
    "terminal": '<path d="M5 7l5 5-5 5"/><path d="M13 17h6"/>',
    "chevron_right": '<path d="M9 6l6 6-6 6"/>',
    "radio_on": '<circle cx="12" cy="12" r="8.2" fill="none" stroke="#2F6FEB" stroke-width="1.8"/>'
                '<circle cx="12" cy="12" r="3.7" fill="#2F6FEB" stroke="none"/>',
    "radio_off": '<circle cx="12" cy="12" r="8.2" fill="none" stroke="#6E6E73" stroke-width="1.8"/>',
    "ring_progress": '<circle cx="12" cy="12" r="9" fill="none" stroke="#3A3A3C" stroke-width="2.4"/>'
                     '<path d="M12 3a9 9 0 0 1 8.5 11.8" fill="none" stroke="#FFFFFF" stroke-width="2.4"/>',
    "read": '<path d="M4 5h7v14H4z"/><path d="M13 5h7v14h-7z"/><path d="M7 9h2"/><path d="M16 9h2"/>',
    "edit": '<path d="M4 20h16"/><path d="M15 4l4 4-9.5 9.5H5.5v-4z"/>',
}
ICON_REG = {}


def r(v):
    return round(float(v), 2)


def svg(name, color="#1D1D1F"):
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
            f'stroke="{color}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'
            f'{ICONS[name]}</svg>\n')


def stack(id, x, y, w, h, kids=None, **kw):
    n = {"t": "stack", "id": id, "x": r(x), "y": r(y), "w": r(w), "h": r(h), "c": kids or []}
    n.update(kw)
    return n


def surface(id, x, y, w, h, *, bg="white", radius=12, border=0, bordercolor=None, kids=None):
    kw = {"variant": "surface", "bg": pal()[bg], "radius": radius}
    if border:
        kw["border"] = border
        if bordercolor:
            kw["bordercolor"] = pal()[bordercolor]
    return stack(id, x, y, w, h, kids, **kw)


def text(id, s, x, y, w, h, *, weight=400, color="ink", size=None, font=None):
    size = size or r(max(h / 1.5, 10.0))
    hh = r(max(h, size * 1.5))
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": hh, "size": size, "line_height": hh, "weight": weight, "color": pal()[color],
            "variant": "single_line", "alignx": 0, "font_src": font or FONT.get(weight, FONT[400])}


def icon(id, name, x, y, w, h, *, color="ink"):
    ICON_REG[id] = (name, color)
    return {"t": "svg", "id": id, "x": r(x), "y": r(y), "w": r(w), "h": r(h), "src": ""}


def code(id, s, x, y, w, h, *, weight=400, color="ink", size=None):
    return text(id, s, x, y, w, h, weight=weight, color=color, size=size, font=MONO)


def flow_text(id, s, x, y, w, h, *, size=14, weight=400, color="ink", font=None,
              line_height=None):
    size = size or r(max(h / 1.5, 10.0))
    lh = line_height if line_height else size * 1.45
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": r(h), "size": size, "line_height": r(lh),
            "weight": weight, "color": pal()[color], "alignx": 0,
            "font_src": font or FONT.get(weight, FONT[400])}


def flow_md(id, s, x, y, w, h, *, size=14, weight=400, color="ink", font=None,
            line_height=None, chip="box"):
    size = size or r(max(h / 1.5, 10.0))
    lh = line_height if line_height else size * 1.45
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": r(h), "size": size, "line_height": r(lh),
            "weight": weight, "color": pal()[color], "bg": pal()[chip], "alignx": 0,
            "variant": "markdown", "font_src": font or FONT.get(weight, FONT[400])}


def input_node(id, x, y, w, h, placeholder, *, size=15, color="muted"):
    return {"t": "input", "id": id, "x": r(x), "y": r(y), "w": r(w), "h": r(h),
            "placeholder": placeholder, "size": size, "color": pal()[color],
            "font_src": FONT[400]}


class Scene:
    """Composes one scene; records icons so the SVGs get written."""
    def __init__(self, num, rows):
        self.num = num
        self.rows = rows
        self.kids = []
        self.controls = {}
        self.inputs = {}
        self.flows = {}

    def b(self, i):
        return self.rows[i][1:]

    def t(self, i):
        return self.rows[i][0]

    def put(self, node):
        self.kids.append(node)

    def add_text(self, id, i, **kw):
        s, x, y, w, h = self.rows[i]
        self.put(text(id, s, x, y, w, h, **kw))

    def add_icon(self, id, name, x, y, w=20, h=20, color="ink"):
        self.put(icon(id, name, x, y, w, h, color=color))

    def add_control(self, id, x, y, w, h, label_i, *, bg="panel", radius=10, border=0,
                    bordercolor=None, lx=None, ly=None, lw=None, lh=None, weight=400,
                    color="ink", icon_name=None, event=None, enabled=1):
        s, sx, sy, sw, sh = self.rows[label_i]
        lx = sx if lx is None else lx
        ly = sy if ly is None else ly
        lw = sw if lw is None else lw
        lh = sh if lh is None else lh
        kids = [surface(id + "_surface", x, y, w, h, bg=bg, radius=radius,
                        border=border, bordercolor=bordercolor),
                {"t": "button", "id": id + "_control", "x": r(x), "y": r(y),
                 "w": r(w), "h": r(h), "enabled": enabled}]
        if icon_name:
            kids.append(icon(id + "_icon", icon_name, x + 10, y + (h - 18) / 2, 18, 18))
        kids.append(text(id + "_label", s, lx, ly, lw, lh, weight=weight, color=color))
        li = len(kids) - 1
        self.put(stack(id, x, y, w, h, kids,
                       kit=json.dumps({"widget": "KitButton",
                                       "bindings": {"control": [1], "label": [li]}})))
        self.controls[id] = (event or id, [int(x), int(y), int(w), int(h)], bool(enabled))

    def add_input(self, id, x, y, w, h, placeholder_i, *, event=None):
        s = self.rows[placeholder_i][0]
        self.put(input_node(id, x, y, w, h, s))
        self.inputs[id] = (event or id, [int(x), int(y), int(w), int(h)])

    def wrap(self, id, x, y, w, h):
        kids = self.kids
        self.kids = []
        self.put(surface(id, x, y, w, h, bg="white", radius=0, kids=kids))

    def wrap_card(self, id, x, y, w, h, *, radius=12):
        kids = self.kids
        self.kids = []
        self.put(surface(id, x, y, w, h, bg="white", radius=radius, border=1,
                         bordercolor="hair", kids=kids))


def load_ocr(num):
    d = json.loads((OCR / f"autonomy-{num:02d}.ocr.json").read_text())
    w, h = d["width"], d["height"]
    sx, sy = 406 / w, 776 / h
    out = []
    for o in d["observations"]:
        x, y, ww, hh = o["bounds"]
        out.append((FIX.get(o["text"], o["text"]), x * sx, y * sy, ww * sx, hh * sy))
    return out


FIX = {
    "Ronsing unittests src/lib.rs (target/debug/deps/octos_cli..)":
        "Running unittests src/lib.rs (target/debug/deps/octos_cli…)",
    "running 12 tests...|": "running 12 tests …",
    "v4-flash7": "v4-flash ▾",
    "v4-flash y": "v4-flash ▾",
    "v4-flash v": "v4-flash ▾",
    "• Live >": "● Live ›",
    "Aside - /btw": "Aside · /btw",
    "are dropped trom the steer queue due to a": "are dropped from the steer queue due to a",
    "metrics. steer_preserved increments and": "metrics.steer_preserved increments and",
    "metrics. steer_dropped remains unchanged.": "metrics.steer_dropped remains unchanged.",
    "% Read crates/octos-core/src/ui_protocol.rs": "⌘ Read crates/octos-core/src/ui_protocol.rs",
    "*Edit crates/octos-cli/tests/steer_queue.rs": "✎ Edit crates/octos-cli/tests/steer_queue.rs",
    "› cargo test -p octos-cli steer_queue": "› cargo test -p octos-cli steer_queue",
}

TITLES = {1: "Review panel", 2: "Autonomy btw", 3: "Context", 4: "Models",
          5: "Product controls", 6: "Review", 7: "Tasks", 8: "Resume",
          9: "Attachments", 10: "Side question",
          11: "Dark conversation", 12: "Dark settings"}
SOURCE = {1: "review-panel", 2: "autonomy-btw", 3: "context", 4: "models",
          5: "product-controls", 6: "review", 7: "tasks", 8: "resume",
          9: "attachments", 10: "side-question",
          11: "dark-conversation", 12: "dark-settings"}


# ------------------------------------------------------------------- scenes
def build_07(sc):
    """TASKS. Outer card (x12..384, y108..756): header 'OctosCode' + 'Tasks',
    a running-task white sub-card (y264..548) with command row + 'Running' pill +
    duration, a grey console block inside it (y350..548, 4 mono lines), a wide
    outlined Cancel button, then a completed row '› cargo clippy -p octos-cli'
    with green 'Done' pill + '1m' (y672..756)."""
    sc.add_text("t01", 1, weight=600, size=19)          # OctosCode
    sc.add_text("t02", 2, weight=600, size=17)          # Tasks
    # running sub-card
    _, cx, cy, cw, ch = sc.rows[3]
    _, rx, ry, rw, rh = sc.rows[4]
    # console block (grey) with 4 mono lines — INSIDE run_card so the extracted
    # task-card component carries the log, not only the command header.
    log_kids = []
    for i, row_i in enumerate([5, 6, 7, 8]):
        s, x, y, w, h = sc.rows[row_i]
        log_kids.append(code(f"t_log{i}", s, x, y, w, h, size=12, color="muted"))
    run_kids = [
        code("t_cmd", sc.t(3), cx, cy, cw, ch, weight=500, size=14),
        surface("run_pill", rx - 6, ry - 4, rw + 12, rh + 8, bg="greenbg", radius=999,
                kids=[text("t_run", sc.t(4), rx, ry, rw, rh, size=12, weight=500, color="green")]),
        text("t_run_dur", "2m", rx + rw + 12, ry, 24, rh, size=12, color="muted"),
        surface("console", 32, 350, 328, 190, bg="box", radius=8, kids=log_kids),
    ]
    _, bx, by, bw, bh = sc.rows[9]
    cancel_row = stack("cancel_row", 40, 552, 288, 44, [
        surface("cancel_surface", 40, 552, 288, 44, bg="white", radius=999,
                border=1, bordercolor="hair"),
        {"t": "button", "id": "cancel_control", "x": 40, "y": 552, "w": 288, "h": 44,
         "enabled": 1},
        text("cancel_label", sc.t(9), bx, by, bw, bh, weight=500),
    ])
    run_kids.append(cancel_row)
    sc.controls["cancel"] = ("task.cancel", [40, 552, 288, 44], True)
    sc.put(surface("run_card", 24, 264, 320, 340, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=run_kids))
    # completed task row (y672..756): command + green Done pill + duration
    _, dx, dy, dw, dh = sc.rows[10]
    sc.put(code("t_done_cmd", sc.t(10), dx, dy, dw, dh, weight=500, size=14))
    _, gx, gy, gw, gh = sc.rows[11]
    sc.put(surface("done_pill", gx - 6, gy - 4, gw + 12, gh + 8, bg="greenbg", radius=999,
                   kids=[text("t_done", sc.t(11), gx, gy, gw, gh, size=12, weight=500, color="green")]))
    sc.add_text("t_dur", 12, color="muted", size=13)
    sc.controls["run_card"] = ("task.open.running", [24, 264, 360, 284], True)
    # outer card (measured edges x12/x384, y108..756) wraps the whole scene
    sc.wrap_card("tasks_card", 12, 108, 372, 648, radius=12)


def build_08(sc):
    """RESUME. Header 'OctosCode', title 'Resume a session', then four session
    rows: a radio marker (row 1 filled dot = current), title, and meta line.
    Row 1 sits on a selected rounded card; hairline dividers between rows."""
    sc.add_text("t01", 1, weight=600, size=19)
    sc.add_text("t02", 2, weight=600, size=17)
    rows = [(3, 4, "row_1"), (5, 6, "row_2"), (7, 8, "row_3"), (9, 10, "row_4")]
    for idx, (title_i, meta_i, cid) in enumerate(rows):
        _, tx, ty, tw, th = sc.rows[title_i]
        _, mx, my, mw, mh = sc.rows[meta_i]
        ytop = ty - 14
        ybot = my + mh + 12
        if idx == 0:
            sc.put(surface(cid + "_sel", 16, ytop, 374, ybot - ytop, bg="sel", radius=10))
        # radio marker at the left gutter
        sc.put(icon(cid + "_radio", "radio_on" if idx == 0 else "radio_off",
                    20, ty + 2, 18, 18, color="ink"))
        sc.put(text(cid + "_title", sc.t(title_i), tx, ty, tw, th, size=15, weight=500))
        sc.put(text(cid + "_meta", sc.t(meta_i), mx, my, mw, mh, size=13, color="muted"))
        sc.controls[cid] = (f"session.resume.{cid}", [16, int(ytop), 374, int(ybot - ytop)], True)
        if idx < 3:
            sc.put(surface(f"div_{idx}", 44, ybot + 4, 346, 1, bg="hair", radius=0))


def build_09(sc):
    """ATTACHMENTS. Two attachment cards: dark code-screenshot thumbnails
    (grey-dark panels with mono placeholder lines), each with an x close button
    top-right; the second shows a 68% progress ring. Size captions '1.2 MB' under
    each, caption row '2 of 4 images • 20 MB max', then the composer
    (input, +, 'Ask for approval' pill, model picker, mic, send)."""
    for k, (card_id, x0) in enumerate([("att_1", 16), ("att_2", 212)]):
        kids = [icon(f"{card_id}_img", "image", x0 + 77, 280, 24, 24, color="muted"),
                surface(f"{card_id}_close_bg", x0 + 146, 196, 24, 24, bg="white", radius=999,
                        kids=[icon(f"{card_id}_close", "x", x0 + 151, 201, 14, 14, color="ink")])]
        if k == 1:
            kids.append(icon("att2_ring", "ring_progress", 274, 282, 52, 52))
            kids.append(text("att2_pct", sc.t(11), 274, 296, 52, 24,
                             size=13, weight=600, color="white"))
        sc.put(surface(card_id, x0, 190, 178, 240, bg="mono", radius=12, border=1,
                       bordercolor="hair", kids=kids))
        sc.controls[f"{card_id}_close"] = (f"attachment.remove.{k+1}",
                                           [x0 + 146, 194, 24, 24], True)
    sc.add_text("t_sz1", 21, size=13, weight=500)
    sc.add_text("t_sz2", 22, size=13, weight=500)
    sc.add_text("t_caption", 23, color="muted", size=13)
    # composer (same component as board 1's composer)
    sc.put(surface("composer", 16, 616, 374, 130, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        input_node("composer_input", 28, 626, 300, 40, sc.t(24)),
        icon("icon_plus", "plus", 26, 726, 18, 24, color="muted"),
        surface("approval_pill", 62, 738, sc.rows[26][3] + 22, sc.rows[26][4] + 12,
                bg="white", radius=999, border=1, bordercolor="hair",
                kids=[text("t_approval", sc.t(26), *sc.rows[26][1:], size=13, color="muted")]),
        text("t_model", FIX.get(sc.t(27), sc.t(27)), *sc.rows[27][1:], size=13, weight=500),
        icon("icon_mic", "mic", 306, 726, 16, 26, color="muted"),
        surface("send_btn", 344, 724, 36, 36, bg="black", radius=999,
                kids=[icon("icon_send", "send", 354, 734, 16, 16, color="white")])]))
    sc.inputs["composer_input"] = ("composer.draft", [28, 626, 300, 40])


def build_10(sc):
    """SIDE QUESTION. User bubble (right-aligned, 2 lines), a tool row
    '⌘ Read crates/octos-core/src/ui_protocol.rs', then an 'Aside · /btw' card
    with 'Dismiss aside' at right, the question title, a 4-line answer region,
    and the composer."""
    # user bubble (black, right-aligned)
    _, x2, y2, w2, h2 = sc.rows[2]
    _, x3, y3, w3, h3 = sc.rows[3]
    bx, by = min(x2, x3) - 14, y2 - 10
    bw = max(x2 + w2, x3 + w3) - bx + 14
    bh = (y3 + h3) - by + 10
    sc.put(surface("user_bubble", bx, by, bw, bh, bg="black", radius=16, kids=[
        text("t_q1", sc.t(2), x2, y2, w2, h2, weight=500, color="white"),
        text("t_q2", sc.t(3), x3, y3, w3, h3, weight=500, color="white")]))
    # tool row: read icon + mono path (board-1 tool-cell shape)
    sc.put(stack("tool_row", 14, sc.rows[4][2] - 8, 320, sc.rows[4][4] + 16, [
        icon("icon_tool", "read", 14, sc.rows[4][2], 20, 20, color="muted"),
        icon("icon_tool_done", "check", 330, sc.rows[4][2] + 2, 16, 16, color="green"),
        code("t_tool", "Read crates/octos-core/src/ui_protocol.rs",
             42, sc.rows[4][2], 280, sc.rows[4][4], size=13, color="muted")]))
    # aside card
    sc.put(surface("aside_card", 16, 340, 374, 250, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=[
        text("t_aside", FIX.get(sc.t(5), sc.t(5)), *sc.rows[5][1:], size=13, weight=600),
        icon("icon_dismiss", "x", sc.rows[6][1] - 20, sc.rows[6][2] + 2, 14, 14, color="muted"),
        text("t_dismiss", sc.t(6), *sc.rows[6][1:], size=12, color="muted"),
        text("t_qtitle", sc.t(7), *sc.rows[7][1:], size=15, weight=600),
    ]))
    sc.controls["dismiss"] = ("aside.dismiss", [int(sc.rows[6][1]), int(sc.rows[6][2]),
                              int(sc.rows[6][3]), int(sc.rows[6][4])], True)
    # 4-line answer as one flowing region
    answer = " ".join(sc.t(i) for i in (8, 9, 10, 11))
    sc.put(flow_text("aside_answer", answer, 26, 452, 330, 130, size=15, color="ink"))
    sc.flows["aside_answer"] = ("aside.answer", 26, 452, 330, 130)
    # composer
    sc.put(surface("composer", 16, 622, 374, 130, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        input_node("composer_input", 26, 632, 300, 40, sc.t(12)),
        icon("icon_plus", "plus", 26, 692, 18, 24, color="muted"),
        surface("approval_pill", 64, 696, sc.rows[14][3] + 22, sc.rows[14][4] + 12,
                bg="white", radius=999, border=1, bordercolor="hair",
                kids=[text("t_approval", sc.t(14), *sc.rows[14][1:], size=13, color="muted")]),
        text("t_model", FIX.get(sc.t(15), sc.t(15)), *sc.rows[15][1:], size=13, weight=500),
        surface("send_btn", 344, 686, 36, 36, bg="black", radius=999,
                kids=[icon("icon_send", "send", 354, 696, 16, 16, color="white")])]))
    sc.inputs["composer_input"] = ("composer.draft", [26, 632, 300, 40])


def build_11(sc):
    """DARK CONVERSATION. Same conversation components on a dark token set:
    header, user bubble, tool row, streaming answer (flow_md), composer."""
    # header
    sc.add_text("t01", 1, weight=600, size=19)
    # user bubble: dark-mode token — a mid-dark pill (panel #2C2C2E) with light ink.
    # NOTE: the map stage re-fits surface fills from the OCR reference region, which
    # sits on the dark page, so the fill may come back near-white; assert the dark
    # panel explicitly in the author so fix_surfaces can restore it.
    _, x2, y2, w2, h2 = sc.rows[2]
    _, x3, y3, w3, h3 = sc.rows[3]
    bx, by = min(x2, x3) - 14, y2 - 10
    bw = max(x2 + w2, x3 + w3) - bx + 14
    bh = (y3 + h3) - by + 10
    sc.put(surface("user_bubble", bx, by, bw, bh, bg="panel", radius=16, kids=[
        text("t_q1", sc.t(2), x2, y2, w2, h2, weight=500, color="ink"),
        text("t_q2", sc.t(3), x3, y3, w3, h3, weight=500, color="ink")]))
    sc.put(code("t_tool", FIX.get(sc.t(4), sc.t(4)), *sc.rows[4][1:], size=13, color="muted"))
    # streaming answer (one flowing markdown region). The markdown variant's `bg`
    # is the INLINE-CODE chip color, not a block fill (design.rs:494-497), so the
    # region needs its own dark surface behind it or the light prose sits on white.
    answer = " ".join(sc.t(i) for i in (5, 6, 7, 8))
    sc.put(surface("answer_surface", 20, 344, 366, 186, bg="page", radius=8, kids=[
        flow_md("answer_md", answer, 30, 352, 340, 170, size=16, line_height=36)]))
    sc.flows["answer_md"] = ("answer.markdown", 30, 352, 340, 170)
    # composer
    sc.put(surface("composer", 16, 616, 374, 130, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        input_node("composer_input", 30, 626, 300, 40, sc.t(9)),
        icon("icon_plus", "plus", 30, 696, 18, 24, color="muted"),
        surface("approval_pill", 76, 700, sc.rows[11][3] + 22, sc.rows[11][4] + 12,
                bg="page", radius=999, border=1, bordercolor="hair",
                kids=[text("t_approval", sc.t(11), *sc.rows[11][1:], size=13, color="muted")]),
        text("t_model", FIX.get(sc.t(12), sc.t(12)), *sc.rows[12][1:], size=13, weight=500),
        surface("send_btn", 344, 690, 36, 36, bg="black", radius=999,
                kids=[icon("icon_send", "send", 354, 700, 16, 16, color="white")])]))
    sc.inputs["composer_input"] = ("composer.draft", [30, 626, 300, 40])


def build_12(sc):
    """DARK SETTINGS. Same settings-group component on the dark token set:
    title 'Settings', grouped card (Connection ● Live › / Workspace octos /
    Profile octos-dev), toggle rows (Desktop notifications / Copy diagnostics),
    plain 'Disconnect'."""
    sc.add_text("t01", 2, weight=600, size=19)
    card_kids = [
        text("t_conn", sc.t(3), *sc.rows[3][1:], size=15, weight=500),
        text("t_live", FIX.get(sc.t(4), sc.t(4)), *sc.rows[4][1:], size=14, weight=500, color="green"),
        text("t_ws", sc.t(5), *sc.rows[5][1:], size=15, weight=500),
        text("t_wsval", sc.t(7), *sc.rows[7][1:], size=14, weight=500, color="muted"),
        text("t_prof", sc.t(6), *sc.rows[6][1:], size=15, weight=500),
        text("t_profval", sc.t(8), *sc.rows[8][1:], size=14, weight=500, color="muted"),
    ]
    sc.put(surface("settings_card", 20, 250, 350, 260, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=card_kids))
    sc.put(surface("div_1", 24, 330, 342, 1, bg="hair", radius=0))
    sc.put(surface("div_2", 24, 404, 342, 1, bg="hair", radius=0))
    sc.controls["conn_row"] = ("settings.connection", [20, 250, 350, 84], True)
    # toggle rows
    sc.add_text("t_notif", 9, size=15, weight=500)
    sc.add_text("t_diag", 10, size=15, weight=500)
    sc.put(surface("toggle1", 320, 498, 50, 30, bg="blue", radius=999,
                   kids=[surface("toggle1_knob", 344, 501, 24, 24, bg="white", radius=999)]))
    sc.put(surface("toggle2", 320, 570, 50, 30, bg="hair", radius=999,
                   kids=[surface("toggle2_knob", 323, 573, 24, 24, bg="white", radius=999)]))
    sc.controls["toggle_notif"] = ("settings.notifications", [320, 498, 50, 30], True)
    sc.controls["toggle_diag"] = ("settings.diagnostics", [320, 570, 50, 30], True)
    sc.add_text("t_disconnect", 11, color="red", weight=500, size=15)
    sc.controls["disconnect"] = ("settings.disconnect", [20, 660, 120, 30], True)


def build_generic(sc):
    """Placeholder for autonomy scenes 1-6 (not my assigned screens): a flat
    text pass so the 8-12-scene manifest validation passes and the pipeline can
    run. Gate-B rendering/review only covers 7-12."""
    for i, (s, x, y, w, h) in enumerate(sc.rows, 1):
        sc.put(text(f"t{i:02d}", FIX.get(s, s), x, y, w, h,
                    weight=600 if i == 1 else 400,
                    color="muted" if i > 1 and h < 14 else "ink"))


BUILDERS = {7: build_07, 8: build_08, 9: build_09, 10: build_10, 11: build_11, 12: build_12}


def build(num):
    global _ACTIVE_C, _ACTIVE_HEX
    dark = num in (11, 12)
    _ACTIVE_C = {**C, **CD} if dark else C
    _ACTIVE_HEX = {**C_HEX, **CD_HEX} if dark else C_HEX
    sc = Scene(num, load_ocr(num))
    BUILDERS.get(num, build_generic)(sc)
    page_bg = "#1C1C1E" if dark else "#FFFFFF"
    doc = {"schema_version": 1, "id": f"autonomy-{num:02d}", "app": "octoscode", "number": num,
           "title": TITLES[num],
           "structure": "Native component reconstructed from the approved autonomy atlas",
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode-dark" if dark else "OctosCode",
                       "page": page_bg, "panel": "#2C2C2E" if dark else "#F7F7F8",
                       "ink": "#F2F2F7" if dark else "#1D1D1F",
                       "muted": "#98989D" if dark else "#6E6E73", "accent": "#2F6FEB"},
           "content_source": "Approved stage-a autonomy atlas + measured Apple Vision OCR bounds",
           "graphics": {},
           "tree": stack("page", 0, 0, 406, 776, sc.kids, variant="surface",
                         bg=(0xFF1C1C1E if dark else C["white"]))}
    return doc, sc


if __name__ == "__main__":
    only = [int(x) for x in sys.argv[1:]] or [7, 8, 9, 10, 11, 12]
    for n in only:
        ICON_REG.clear()
        doc, sc = build(n)
        d = ROOT / "cards" / f"autonomy-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        for iid, (name, color) in ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(svg(name, hexpal().get(color, "#1D1D1F")))
        controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                    for cid, (ev, b, en) in sc.controls.items()}
        for cid, (ev, b) in sc.inputs.items():
            controls[cid] = {"event": ev, "source_bounds": b, "enabled": True}
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": controls, "source": SOURCE[n]}, indent=2) + "\n")
        print(f"wrote autonomy-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(ICON_REG)} icons, {len(controls)} controls")
