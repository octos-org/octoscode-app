#!/usr/bin/env python3
"""Author the 12 conversation contracts in the design-flow's STANDARD pattern.

Why a rewrite (card #11b): the first batch was a FLAT tree (page -> text/button).
`octoscript-makepad/crates/octoscript-makepad/src/design.rs:292` emits a node's
children ONLY inside the `NodeKind::Stack` arm, so a label nested in a `button`
node was dropped, and a flat tree has no surface/icons at all. The canonical
pattern (proven by examples/school/cards/school-02, which renders correctly) is:

    stack(id, KIT)                     <- control/surface container (Stack)
      stack(id_surface, variant=surface) <- the fill
      button(id_control)                 <- the native control
      text(id_label)                     <- copy as a Stack child
      svg(id_icon)                       <- line icons

Text copy + positions are MEASURED (Apple Vision OCR on each scene's
reference.png, logical 406x776) and corrected only where the approved prompt
names a different string. Composition is INDEX-DRIVEN off the OCR rows, so a
glyph-confused OCR string can never crash or silently drop a node.

Run: python3 tools/author_v2.py     # writes contract.json + assets/*.svg
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OCR = ROOT / "ocr"

C = {"white": 0xFFFFFFFF, "panel": 0xFFF7F7F8, "hair": 0xFFE5E5E7, "ink": 0xFF1D1D1F,
     "muted": 0xFF6E6E73, "black": 0xFF000000, "blue": 0xFF2F6FEB, "green": 0xFF1F883D,
     "greenbg": 0xFFE6F4EA, "red": 0xFFCF222E, "redbg": 0xFFFDECEC, "sel": 0xFFF2F2F7,
     "mono": 0xFFF6F6F7}
C_HEX = {"white": "#FFFFFF", "panel": "#F7F7F8", "hair": "#E5E5E7", "ink": "#1D1D1F",
         "muted": "#6E6E73", "black": "#000000", "blue": "#2F6FEB", "green": "#1F883D",
         "greenbg": "#E6F4EA", "red": "#CF222E", "redbg": "#FDECEC", "sel": "#F2F2F7",
         "mono": "#F6F6F7"}
FONT = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf",
        600: "self:resources/ux/Inter-600.ttf", 700: "self:resources/ux/Inter-700.ttf"}
# No monospace face ships in the host's resource set (verified: only Inter/Roboto/
# Montserrat/DMSans/NotoSansSC/Poppins/PlusJakarta). Code-ish copy uses Inter-400.
MONO = FONT[400]

TITLES = {1: "Thread list", 2: "New chat", 3: "Streaming turn", 4: "Tool cells",
          5: "Inline approval", 6: "User question", 7: "Edited files",
          8: "Composer states", 9: "Completed answer", 10: "Goal and plan",
          11: "Review diff", 12: "Settings card"}

ICONS = {
    "bell": '<path d="M6 9a6 6 0 0 1 12 0c0 5 2 6 2 6H4s2-1 2-6"/><path d="M10 20a2 2 0 0 0 4 0"/>',
    "search": '<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15l5 5"/>',
    "compose": '<path d="M4 20h16"/><path d="M15 4l4 4-9.5 9.5H5.5v-4z"/>',
    "fork": '<circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="8" r="2"/>'
            '<path d="M6 7v10"/><path d="M18 10c0 4-6 2.5-6 7"/>',
    "spinner": '<path d="M12 3a9 9 0 1 0 9 9"/>',
    "plus": '<path d="M12 5v14"/><path d="M5 12h14"/>',
    "stop": '<rect x="6" y="6" width="12" height="12" rx="2" fill="#FFFFFF" stroke="none"/>',
    "send": '<path d="M5 12h13"/><path d="M12 6l6 6-6 6"/>',
    "file": '<path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4"/>',
    "terminal": '<path d="M5 7l5 5-5 5"/><path d="M13 17h6"/>',
    "check": '<path d="M5 12.5l4.5 4.5L19 7"/>',
    "copy": '<rect x="8" y="8" width="11" height="12" rx="2"/><path d="M5 16V5h11"/>',
    "thumbs": '<path d="M7 11v9H4v-9z"/><path d="M7 11l4-8a2 2 0 0 1 2 2v5h5a2 2 0 0 1 2 2l-1.6 6H7"/>',
    "share": '<path d="M12 16V4"/><path d="M8 8l4-4 4 4"/><path d="M5 14v5h14v-5"/>',
    "shield": '<path d="M12 3l7 3v6c0 4.5-3 7.8-7 9-4-1.2-7-4.5-7-9V6z"/>',
    "chevron": '<path d="M9 6l6 6-6 6"/>',
    "pause": '<rect x="8" y="6" width="3" height="12" rx="1"/><rect x="13" y="6" width="3" height="12" rx="1"/>',
    "bell_dot": '<path d="M6 9a6 6 0 0 1 12 0c0 5 2 6 2 6H4s2-1 2-6"/>'
                '<circle cx="17" cy="5" r="2" fill="#CF222E" stroke="none"/>',
    "mic": '<rect x="9" y="2.6" width="6" height="11" rx="3"/>'
           '<path d="M5.5 11a6.5 6.5 0 0 0 13 0"/><path d="M12 17.5V21"/><path d="M8.5 21h7"/>',
}

def r(v):
    return round(float(v), 2)

def svg(name, color="#1D1D1F"):
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
            f'stroke="{color}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'
            f'{ICONS[name]}</svg>\n')

# ------------------------------------------------------------------- node dsl
def stack(id, x, y, w, h, kids=None, **kw):
    n = {"t": "stack", "id": id, "x": r(x), "y": r(y), "w": r(w), "h": r(h), "c": kids or []}
    n.update(kw)
    return n

def surface(id, x, y, w, h, *, bg="white", radius=12, border=0, bordercolor=None, kids=None):
    kw = {"variant": "surface", "bg": C[bg], "radius": radius}
    if border:
        kw["border"] = border
        if bordercolor:
            kw["bordercolor"] = C[bordercolor]
    return stack(id, x, y, w, h, kids, **kw)

def text(id, s, x, y, w, h, *, weight=400, color="ink", size=None, font=None):
    size = size or r(max(h / 1.5, 10.0))
    hh = r(max(h, size * 1.5))
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": hh, "size": size, "line_height": hh, "weight": weight, "color": C[color],
            "variant": "single_line", "alignx": 0, "font_src": font or FONT.get(weight, FONT[400])}

ICON_REG = {}   # id -> (name, color); populated by icon() so NO svg can be missed

def icon(id, name, x, y, w, h, *, color="ink"):
    ICON_REG[id] = (name, color)
    return {"t": "svg", "id": id, "x": r(x), "y": r(y), "w": r(w), "h": r(h), "src": ""}

def input_node(id, x, y, w, h, placeholder, *, size=15, color="muted"):
    return {"t": "input", "id": id, "text": "", "placeholder": placeholder,
            "x": r(x), "y": r(y), "w": r(w), "h": r(max(h, size * 1.5)), "size": size,
            "line_height": r(max(h, size * 1.5)), "weight": 400, "color": C[color],
            "variant": "single_line", "alignx": 0, "font_src": FONT[400], "enabled": 1, "c": []}

class Scene:
    """Composes one scene; records icons so the SVGs get written."""
    def __init__(self, num, rows):
        self.num = num
        self.rows = rows          # [(text,x,y,w,h)] measured, in OCR order
        self.kids = []
        self.icons = {}
        self.controls = {}        # id -> (event, bounds, enabled)
        self.inputs = {}
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

def load_ocr(num):
    d = json.loads((OCR / f"conversation-{num:02d}.ocr.json").read_text())
    w, h = d["width"], d["height"]
    sx, sy = 406 / w, 776 / h
    out = []
    for o in d["observations"]:
        x, y, ww, hh = o["bounds"]
        out.append((FIX.get(o["text"], o["text"]), x * sx, y * sy, ww * sx, hh * sy))
    return out

FIX = {"Review PR #2556": "Review PR #2566", "Bump octos-core to abea8505": "Bump octos-core to a6ea8505",
       "v4-flash v": "v4-flash ▾", "V4-flash v": "v4-flash ▾", "OctosCode v": "OctosCode ▾",
       "handled across reconnects...": "handled across reconnects…",
       "l'Il run tests to confirm the fix": "I'll run tests to confirm the fix",
       "and update the affected code... |": "and update the affected code…",
       "1 queued • Steer now • X": "1 queued · Steer now · ✕", "Sep 28,9:41 PM": "Sep 28, 9:41 PM",
       "• Fixed loss of queuedsteers when": "• Fixed loss of queued steers when",
       "• All tests pass: 12 passed .": "• All tests pass: 12 passed.",
       "• Changes included in commit a6ea8505 .": "• Changes included in commit a6ea8505.",
       "• Updated ui_protocol_transport.rs": "• Updated ui_protocol_transport.rs"}

def fix(s):
    return FIX.get(s, s)

# ------------------------------------------------------------------- scenes
def build_01(sc):
    sc.add_text("t01", 0, weight=600, size=19)
    sc.add_icon("icon_bell", "bell_dot", 306, 46, 21, 25)
    sc.add_icon("icon_search", "search", 359, 46, 22, 25)
    sc.add_control("new_chat", 16, 120, 374, 44, 1, bg="panel", radius=10, weight=500,
                   border=1, bordercolor="hair", lx=38, ly=132, lw=140, lh=24, event="thread.new")
    # the compose icon belongs at the ROW'S RIGHT EDGE (atlas x=348), not left of the label
    sc.add_icon("icon_compose", "compose", 347, 130, 24, 28)
    rows = [(2, "thread_1", "sel", 500), (3, "thread_2", "white", 400), (4, "thread_3", "white", 400),
            (5, "thread_4", "white", 400), (6, "thread_5", "white", 400)]
    for i, cid, bg, wt in rows:
        _, x, y, w, h = sc.rows[i]
        sc.add_control(cid, 16, y - 14, 374, h + 28, i, bg=bg, radius=10, weight=wt,
                       lx=x, ly=y, lw=300, lh=h, event="thread.open")
    _, x3, y3, _, _ = sc.rows[3]
    sc.add_icon("icon_fork", "fork", 352, y3 - 3, 19, 26, color="muted")

def build_03(sc):
    _, x1, y1, w1, h1 = sc.rows[0]
    _, x2, y2, w2, h2 = sc.rows[1]
    bx, by = min(x1, x2) - 14, y1 - 12
    bw = max(x1 + w1, x2 + w2) - bx + 14
    bh = (y2 + h2) - by + 12
    sc.put(surface("user_bubble", bx, by, bw, bh, bg="black", radius=16, kids=[
        text("t01", sc.t(0), x1, y1, w1, h1, weight=500, color="white"),
        text("t02", sc.t(1), x2, y2, w2, h2, weight=500, color="white")]))
    sc.add_icon("icon_spinner", "spinner", 44, 198, 18, 18, color="muted")
    sc.add_text("t03", 2, color="muted", weight=500)
    for i, tid in [(3, "t04"), (4, "t05"), (5, "t06"), (6, "t07")]:
        sc.add_text(tid, i)
    _, x8, y8, w8, h8 = sc.rows[7]
    chips = []
    for i, cid in [(7, "chip_ws"), (8, "chip_mode"), (9, "chip_branch")]:
        s, x, y, w, h = sc.rows[i]
        chips.append(surface(cid, x - 8, y - 6, w + 16, h + 12, bg="panel", radius=999,
                             border=1, bordercolor="hair",
                             kids=[text(f"t{i+1:02d}", s, x, y, w, h, size=13, weight=500)]))
    sc.put(stack("chips", 24, y8 - 8, 358, h8 + 16, chips))
    _, px, py, pw, ph = sc.rows[10]
    ax, ay, aw, ah = sc.rows[12][1:]
    kids = [input_node("composer_input", 30, 594, 300, 40, sc.t(10)),
            icon("icon_plus", "plus", 28, 685, 18, 24, color="muted"),
            surface("approval_pill", ax - 10, ay - 6, aw + 20, ah + 13, bg="white",
                    radius=999, border=1, bordercolor="hair",
                    kids=[text("t13", sc.t(12), ax, ay, aw, ah, size=13, color="muted")]),
            icon("icon_mic", "mic", 309, 681, 17, 27, color="muted"),
            text("t14", fix(sc.t(13)), *sc.rows[13][1:], size=13, weight=500),
            surface("stop_btn", 346, 674, 36, 36, bg="black", radius=999,
                    kids=[icon("icon_stop", "stop", 356, 684, 16, 16)])]
    sc.put(surface("composer", 16, 584, 374, 150, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=kids))
    sc.inputs["composer_input"] = ("composer.draft", [30, 594, 300, 40])

def build_04(sc):
    _, x0, y0, w0, h0 = sc.rows[0]
    sc.put(surface("tool_1", 16, y0 - 12, 374, 56, bg="panel", radius=10, kids=[
        icon("icon_file", "file", 26, y0 - 2, 18, 18),
        text("t01", sc.t(0), x0, y0, w0, h0, weight=500, size=14),
        text("t02", sc.t(1), *sc.rows[1][1:], size=13, color="muted"),
        icon("icon_check1", "check", 356, y0 - 2, 18, 18, color="green")]))
    _, x2, y2, w2, h2 = sc.rows[2]
    sc.put(surface("tool_2", 16, y2 - 12, 374, 56, bg="panel", radius=10, kids=[
        icon("icon_search2", "search", 26, y2 - 2, 18, 18),
        text("t03", sc.t(2), x2, y2, w2, h2, weight=500, size=14),
        text("t04", sc.t(3), *sc.rows[3][1:], size=13, color="muted"),
        icon("icon_check2", "check", 356, y2 - 2, 18, 18, color="green")]))
    _, x4, y4, w4, h4 = sc.rows[4]
    sc.put(surface("tool_3", 16, y4 - 12, 374, 62, bg="panel", radius=10, kids=[
        icon("icon_term", "terminal", 26, y4 + 2, 18, 18),
        text("t05", sc.t(4), x4, y4, w4, h4, weight=500, size=14),
        text("t06", sc.t(5), *sc.rows[5][1:], size=14, weight=500),
        icon("icon_check3", "check", 356, y4 - 2, 18, 18, color="green")]))
    sc.put(surface("tool_3_output", 16, 424, 374, 180, bg="mono", radius=10, kids=[
        text("t07", sc.t(6), *sc.rows[6][1:], size=13, color="muted"),
        text("t08", sc.t(8), *sc.rows[8][1:], size=13),
        text("t09", sc.t(9), *sc.rows[9][1:], size=13, weight=500, color="green")]))

def build_08(sc):
    _, px, py, pw, ph = sc.rows[0]
    ax0, ay0, aw0, ah0 = sc.rows[2][1:]
    sc.put(surface("composer_idle", 16, 140, 374, 160, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        input_node("composer_idle_input", 30, 150, 300, 40, sc.t(0)),
        icon("icon_plus1", "plus", 30, 262, 18, 24, color="muted"),
        surface("approval_pill1", ax0 - 10, ay0 - 6, aw0 + 20, ah0 + 13, bg="white",
                radius=999, border=1, bordercolor="hair",
                kids=[text("t03", sc.t(2), ax0, ay0, aw0, ah0, size=13, color="muted")]),
        icon("icon_mic1", "mic", 306, 259, 16, 27, color="muted"),
        text("t04", fix(sc.t(3)), *sc.rows[3][1:], size=13, weight=500),
        surface("send1", 344, 252, 36, 36, bg="black", radius=999,
                kids=[icon("icon_send", "send", 354, 262, 16, 16)])]))
    sc.inputs["composer_idle_input"] = ("composer.draft", [30, 150, 300, 40])
    _, qx, qy, qw, qh = sc.rows[4]
    sc.put(surface("queued_row", 16, qy - 10, qw + 28, qh + 20, bg="panel", radius=10,
                   kids=[text("t05", fix(sc.t(4)), qx, qy, qw, qh, size=13, weight=500)]))
    ax1, ay1, aw1, ah1 = sc.rows[7][1:]
    sc.put(surface("composer_active", 16, 480, 374, 170, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        text("t06", sc.t(5), *sc.rows[5][1:], size=15),
        icon("icon_plus2", "plus", 28, 598, 18, 24, color="muted"),
        surface("approval_pill2", ax1 - 10, ay1 - 6, aw1 + 20, ah1 + 13, bg="white",
                radius=999, border=1, bordercolor="hair",
                kids=[text("t08", sc.t(7), ax1, ay1, aw1, ah1, size=13, color="muted")]),
        icon("icon_mic2", "mic", 306, 598, 16, 26, color="muted"),
        text("t09", fix(sc.t(8)), *sc.rows[8][1:], size=13, weight=500),
        surface("stop2", 346, 590, 36, 36, bg="black", radius=999,
                kids=[icon("icon_stop2", "stop", 356, 600, 16, 16)])]))

def build_09(sc):
    sc.add_control("worked_row", 16, 32, 374, 44, 0, bg="panel", radius=10, weight=500,
                   color="muted", icon_name="chevron",
                   lx=sc.rows[0][1], ly=sc.rows[0][2], lw=280, lh=sc.rows[0][4], event="turn.expand")
    sc.add_text("t02", 1, weight=600, size=16)
    for i in range(2, 10):
        sc.add_text(f"t{i+1:02d}", i, size=14)
    sc.add_icon("icon_copy", "copy", 24, 640, 20, 20, color="muted")
    sc.add_icon("icon_thumbs", "thumbs", 52, 640, 20, 20, color="muted")
    sc.add_icon("icon_share", "share", 80, 640, 20, 20, color="muted")
    sc.add_text("t11", 10, color="muted", size=13)

MUTED = {"• 412 lines", "• 7 matches", "Working • 12s", "Ask for approval", "running 12 tests",
         "Sep 28, 9:41 PM", "Sep 28,9:41 PM", "+", "••", "Skip", "Deny", "v4-flash ▾", "V4-flash v",
         "Worked for 3m 4s ›", "Ask Octos anything"}

def build_generic(sc):
    for i, (s, x, y, w, h) in enumerate(sc.rows, 1):
        sc.put(text(f"t{i:02d}", fix(s), x, y, w, h,
                    weight=600 if i == 1 else 400,
                    color="muted" if s in MUTED else "ink"))

BUILDERS = {1: build_01, 3: build_03, 4: build_04, 8: build_08, 9: build_09}
SOURCE = {1: "thread-list", 2: "new-chat", 3: "streaming-turn", 4: "tool-cells",
          5: "inline-approval", 6: "user-question", 7: "edited-files", 8: "composer-states",
          9: "completed-answer", 10: "goal-plan", 11: "review-diff", 12: "settings"}

def build(num):
    sc = Scene(num, load_ocr(num))
    BUILDERS.get(num, build_generic)(sc)
    doc = {"schema_version": 1, "id": f"conversation-{num:02d}", "app": "octoscode", "number": num,
           "title": TITLES[num],
           "structure": "Native component reconstructed from the approved conversation atlas",
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                       "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
           "content_source": "Approved stage-a atlas + measured Apple Vision OCR bounds",
           "graphics": {},
           "tree": stack("page", 0, 0, 406, 776, sc.kids, variant="surface", bg=C["white"])}
    return doc, sc

if __name__ == "__main__":
    for n in range(1, 13):
        ICON_REG.clear()
        doc, sc = build(n)
        d = ROOT / "cards" / f"conversation-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        for iid, (name, color) in ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(svg(name, C_HEX.get(color, "#1D1D1F")))
        # service-actions.json: every native control + input, from the same authoring pass
        controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                    for cid, (ev, b, en) in sc.controls.items()}
        for cid, (ev, b) in sc.inputs.items():
            controls[cid] = {"event": ev, "source_bounds": b, "enabled": True}
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": controls, "source": SOURCE[n]}, indent=2) + "\n")
        print(f"wrote conversation-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(ICON_REG)} icons, {len(controls)} controls")
