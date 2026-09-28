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
     "mono": 0xFFF6F6F7, "box": 0xFFF4F4F5}
C_HEX = {"white": "#FFFFFF", "panel": "#F7F7F8", "hair": "#E5E5E7", "ink": "#1D1D1F",
         "muted": "#6E6E73", "black": "#000000", "blue": "#2F6FEB", "green": "#1F883D",
         "greenbg": "#E6F4EA", "red": "#CF222E", "redbg": "#FDECEC", "sel": "#F2F2F7",
         "mono": "#F6F6F7", "box": "#F4F4F5"}
FONT = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf",
        600: "self:resources/ux/Inter-600.ttf", 700: "self:resources/ux/Inter-700.ttf"}
# Card #11d: the host now bundles a monospace face. `apps/kit-host/resources/ux/
# LiberationMono-Regular.ttf` is copied from the pinned makepad tree
# (makepad/widgets/resources/LiberationMono-Regular.ttf) and beauty-host is rebuilt
# from that clone, so `self:resources/ux/LiberationMono-Regular.ttf` resolves
# (verified: 0 "not available in this build" warnings). See tools/rebuild.sh.
MONO = "self:resources/ux/LiberationMono-Regular.ttf"

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
    # Card #18 (after view_image): the approval header shield carries a CHECK
    # inside its outline; the plan's done steps are a BLUE FILLED disc with a
    # white check; in-progress is a dotted ring; pending is a plain outline ring.
    "shield_check": '<path d="M12 3l7 3v6c0 4.5-3 7.8-7 9-4-1.2-7-4.5-7-9V6z"/>'
                    '<path d="M9 11.5l2.2 2.2L15 9.5"/>',
    "check_circle": '<circle cx="12" cy="12" r="8.5" fill="#2F6FEB" stroke="none"/>'
                    '<path d="M8.3 12.3l2.4 2.4L15.7 9.7" stroke="#FFFFFF" stroke-width="1.9" '
                    'fill="none"/>',
    "ring_dotted": '<circle cx="12" cy="12" r="8.0" fill="none" stroke-dasharray="1.6 3"/>',
    "ring": '<circle cx="12" cy="12" r="8.0" fill="none"/>',
    "stop_filled": '<rect x="6.5" y="6.5" width="11" height="11" rx="1.5" '
                   'fill="#6E6E73" stroke="none"/>',
    # 06 header icon is a "?" in a CIRCLE (ref-06-hdr), and its options are
    # radios: a ring with a filled dot when chosen, a bare ring when not.
    "question_circle": '<circle cx="12" cy="12" r="9"/>'
                       '<path d="M9.4 9.2a2.7 2.7 0 1 1 3.3 2.9c-.6.2-.9.7-.9 1.3v.6"/>'
                       '<circle cx="11.8" cy="17" r="0.9" fill="#1D1D1F" stroke="none"/>',
    "radio_on": '<circle cx="12" cy="12" r="8.2" fill="none" stroke="#2F6FEB" stroke-width="1.8"/>'
                '<circle cx="12" cy="12" r="3.7" fill="#2F6FEB" stroke="none"/>',
    "radio_off": '<circle cx="12" cy="12" r="8.2" fill="none" stroke="#C7C7CC" stroke-width="1.6"/>',
    "chevron": '<path d="M9 6l6 6-6 6"/>',
    # Card #18b: the atlas draws the pause as two FILLED bars (the first pass used
    # stroke-only rects, which rendered as two thin outlines).
    "pause": '<rect x="7.2" y="5.6" width="3.7" height="12.8" rx="1.1" fill="#6E6E73" '
             'stroke="none"/><rect x="13.1" y="5.6" width="3.7" height="12.8" rx="1.1" '
             'fill="#6E6E73" stroke="none"/>',
    "bell_dot": '<path d="M6 9a6 6 0 0 1 12 0c0 5 2 6 2 6H4s2-1 2-6"/>'
                '<circle cx="17" cy="5" r="2" fill="#CF222E" stroke="none"/>',
    "mic": '<rect x="9" y="2.6" width="6" height="11" rx="3"/>'
           '<path d="M5.5 11a6.5 6.5 0 0 0 13 0"/><path d="M12 17.5V21"/><path d="M8.5 21h7"/>',
    "undo": '<path d="M9 7h6a5 5 0 0 1 0 10h-6"/><path d="M12 4L9 7l3 3"/>',
    "chevron_down": '<path d="M6 9l6 6 6-6"/>',
    "chevron_right": '<path d="M9 6l6 6-6 6"/>',
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

def code(id, s, x, y, w, h, *, weight=400, color="ink", size=None):
    """A code/path/command line: the bundled monospace face."""
    return text(id, s, x, y, w, h, weight=weight, color=color, size=size, font=MONO)

def flow_text(id, s, x, y, w, h, *, size=14, weight=400, color="ink", font=None,
              line_height=None):
    """A dynamic text region: one native flowing Label bound to one data id.

    Card #11e / LESSONS "Dynamic content is a flow region": runtime prose must not
    be boxes placed at measured coordinates. `text()` sets `variant: single_line`,
    which design.rs (octoscript-makepad/crates/octoscript-makepad/src/design.rs:338)
    lowers to the non-wrapping `flow: Right`. Omitting the variant and stating a
    height taller than one line leaves makepad's Label at its default
    `Flow::right_wrap()` (makepad/widgets/src/label.rs:240), so it reflows.
    """
    size = size or r(max(h / 1.5, 10.0))
    lh = line_height if line_height else size * 1.45
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": r(h), "size": size, "line_height": r(lh),
            "weight": weight, "color": C[color], "alignx": 0,
            "font_src": font or FONT.get(weight, FONT[400])}

def flow_md(id, s, x, y, w, h, *, size=14, weight=400, color="ink", font=None,
            line_height=None, chip="box"):
    """A dynamic MARKDOWN prose region: one native flowing widget whose inline
    `code` spans draw as the kit's grey rounded mono chip (card #11f).

    `variant: "markdown"` lowers (design.rs) to makepad's `Markdown` — the only
    reachable widget with both a declarative body AND an inline-code draw hook
    (makepad/widgets/src/markdown.rs:118-176: `inline_code_padding`,
    `inline_code_margin`, `text_style_fixed`, `draw_block.code_color`; the chip
    box is drawn at makepad/widgets/src/text_flow.rs:2201). `Markdown` parses the
    backticks in `body` and draws each span with the mono `text_style_fixed` and
    a `draw_block.code_color` background — `bg` here — IN THE SAME flow as the
    prose. A plain `Label` is a single style, so it cannot carry a chip.
    """
    size = size or r(max(h / 1.5, 10.0))
    lh = line_height if line_height else size * 1.45
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": r(h), "size": size, "line_height": r(lh),
            "weight": weight, "color": C[color], "bg": C[chip], "alignx": 0,
            "variant": "markdown", "font_src": font or FONT.get(weight, FONT[400])}

def dots(id, x, y, w, h, *, n=15, size=13, color="ink", track=4.0):
    """A dotted progress line: n round dots, pitch set by tracking.

    `semantics.py:327-331` rejects a text box shorter than its own line box as
    clipping, so `h` is widened to the line box here rather than trusted from the
    caller (the atlas dot ink is only ~4px tall, but the glyph needs a full line).
    """
    lh = max(h, size * 1.35)
    return {"t": "text", "id": id, "text": "·" * n, "x": r(x), "y": r(y - (lh - h) / 2),
            "w": r(w), "h": r(lh), "size": size, "line_height": r(lh), "weight": 700,
            "color": C[color], "variant": "single_line", "alignx": 0,
            "tracking": track, "font_src": FONT[700]}

def chip(id, s, x, y, w, h, *, color="ink", size=None, weight=400, radius=6, padx=4, pady=3):
    """An inline code chip: a grey rounded surface wrapping a mono token."""
    size = size or r(max(h / 1.5, 9.0))
    inner = text(id + "_t", s, x, y, w, h, weight=weight, color=color, size=size, font=MONO)
    return surface(id, x - padx, y - pady, w + 2 * padx, h + 2 * pady,
                   bg="mono", radius=radius, kids=[inner])

def row_with_chip(id, s, box, chip_box, token, *, size=13, color="ink"):
    """Split a measured OCR line into prefix / code chip / suffix at the token."""
    x, y, w, h = box
    cx, cy, cw, ch = chip_box
    pre, sep, post = s.partition(token)
    out = []
    if pre:
        out.append(text(id + "a", pre, x, y, max(cx - x, 8), h, size=size, color=color))
    if sep:
        out.append(chip(id + "c", token, cx, cy, cw, ch, size=size, color=color))
    if post:
        out.append(text(id + "b", post, cx + cw, y, max((x + w) - (cx + cw), 8), h,
                        size=size, color=color))
    return out

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
        self.flows = {}           # id -> (data id, bounds) for flowing text regions
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
        """Reparent every node placed so far into ONE `stack` root — the design
        flow's "native composition root" (card #16) — so the subtree can be
        extracted as a reusable component.

        The root carries a WHITE fill (card #18): in-scene it is appearance-inert
        (makepad `abs_pos` is window-absolute, design.rs:200, and the page behind it
        is already white), but as a STANDALONE component the host has no page to
        paint the ground, so a fill-less root renders on the host's dark default
        (#4c4c4c). `wrap_card` is the variant that also draws a border.
        """
        kids = self.kids
        self.kids = []
        self.put(surface(id, x, y, w, h, bg="white", radius=0, kids=kids))

    def wrap_card(self, id, x, y, w, h, *, radius=12):
        """Like `wrap`, but the root is a bordered white SURFACE — the enclosing
        card the reference draws around some groups (e.g. 11's diff)."""
        kids = self.kids
        self.kids = []
        self.put(surface(id, x, y, w, h, bg="white", radius=radius, border=1,
                         bordercolor="hair", kids=kids))

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

STREAM_MD = ("I'm tracing how queued steers are handled across reconnects…\n"
             "\n"
             # Card #11f: one backticked token proves 03's streaming paragraphs
             # render inline code the same way as 09's answer body.
             "I'll run tests to confirm the fix and update the affected `steer_dropped` path…")

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
    # Card #11f: same flowing widget as 09, so 03's streaming paragraphs render an
    # inline `code` span as the kit chip too.
    sc.put(flow_md("assistant_md", STREAM_MD, 21, 248, 358, 176, size=17.5, line_height=35))
    sc.flows["assistant_md"] = ("timeline.assistant.markdown", 21, 248, 358, 176)
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
    """Three tool cells. Atlas (measured): each row is a bordered card;
    tool_3's card CONTAINS its output box. Code/paths/commands use the mono face."""
    _, x0, y0, w0, h0 = sc.rows[0]
    sc.put(surface("tool_1", 16, 54, 374, 92, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=[
        icon("icon_file", "file", 42, 78, 24, 26),
        code("t01", sc.t(0), x0, y0, w0, h0, weight=500, size=14),
        text("t02", sc.t(1), *sc.rows[1][1:], size=13, color="muted"),
        icon("icon_check1", "check", 356, y0 - 2, 20, 20, color="green")]))
    _, x2, y2, w2, h2 = sc.rows[2]
    sc.put(surface("tool_2", 16, 184, 374, 92, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=[
        icon("icon_search2", "search", 40, 206, 24, 24),
        code("t03", sc.t(2), x2, y2, w2, h2, weight=500, size=14),
        text("t04", sc.t(3), *sc.rows[3][1:], size=13, color="muted"),
        icon("icon_check2", "check", 356, y2 - 2, 20, 20, color="green")]))
    _, x4, y4, w4, h4 = sc.rows[4]
    output = surface("tool_3_output", 24, 398, 358, 250, bg="box", radius=8, kids=[
        code("t07", sc.t(6), *sc.rows[6][1:], size=13, color="muted"),
        dots("t08", 46, 489, 112, 8),                    # the full dotted progress line
        code("t09", sc.t(8), *sc.rows[8][1:], size=13),
        code("t10", sc.t(9), *sc.rows[9][1:], size=13, weight=500, color="green")])
    # the terminal glyph is the icon; the ">_ " the OCR read is that icon itself
    sc.put(surface("tool_3", 16, 314, 374, 356, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=[
        icon("icon_term", "terminal", 40, 338, 22, 22),
        # the OCR row's x included the ">_ " icon glyph; the label starts AFTER the icon
        code("t05", "Ran cargo test -p octos-cli", 87.5, y4, w4, h4, weight=500, size=14),
        code("t06", sc.t(5), *sc.rows[5][1:], size=14, weight=500),
        icon("icon_check3", "check", 356, y4 - 2, 20, 20, color="green"),
        output]))

def build_08(sc):
    """Two composer states, each a bordered card whose edges read as the divider
    (atlas: full-width rules at y=320.5 and y=475.5). The send arrow is white on black."""
    sc.put(surface("composer_idle", 16, 140, 374, 180, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        input_node("composer_idle_input", 30, 150, 300, 40, sc.t(0)),
        icon("icon_plus1", "plus", 30, 262, 18, 24, color="muted"),
        *chip_row(sc, "pill1", 2, pad=10, bg="white"),
        icon("icon_mic1", "mic", 306, 259, 16, 27, color="muted"),
        text("t04", fix(sc.t(3)), *sc.rows[3][1:], size=13, weight=500),
        surface("send1", 344, 252, 36, 36, bg="black", radius=999,
                kids=[icon("icon_send", "send", 354, 262, 16, 16, color="white")])]))
    sc.inputs["composer_idle_input"] = ("composer.draft", [30, 150, 300, 40])
    _, qx, qy, qw, qh = sc.rows[4]
    sc.put(surface("queued_row", 16, qy - 10, qw + 28, qh + 20, bg="panel", radius=10,
                   kids=[text("t05", fix(sc.t(4)), qx, qy, qw, qh, size=13, weight=500)]))
    sc.put(surface("composer_active", 16, 476, 374, 176, bg="panel", radius=12, border=1,
                   bordercolor="hair", kids=[
        text("t06", sc.t(5), *sc.rows[5][1:], size=15),
        icon("icon_plus2", "plus", 28, 598, 18, 24, color="muted"),
        *chip_row(sc, "pill2", 7, pad=10, bg="white"),
        icon("icon_mic2", "mic", 306, 598, 16, 26, color="muted"),
        text("t09", fix(sc.t(8)), *sc.rows[8][1:], size=13, weight=500),
        surface("stop2", 346, 590, 36, 36, bg="black", radius=999,
                kids=[icon("icon_stop2", "stop", 356, 600, 16, 16)])]))

def chip_row(sc, id, row_i, *, pad=10, bg="white"):
    """The 'Ask for approval' pill: a bordered surface wrapping measured copy."""
    x, y, w, h = sc.rows[row_i][1:]
    return [surface(id, x - pad, y - 6, w + 2 * pad, h + 13, bg=bg, radius=999,
                    border=1, bordercolor="hair",
                    kids=[text(id + "_t", sc.t(row_i), x, y, w, h, size=13, color="muted")])]

# The atlas renders this as a LOOSE list: a paragraph gap between bullets
# (measured pitch 60.5 / 38.5 / 44 / 65.5 / 43 / 41 / 65.0 / 64.5), so each item
# is separated by a blank line rather than packed tight.
ANSWER_MD = ("Queued steers now survive a reconnect.\n"
             "\n"
             "• Fixed loss of queued steers when reconnecting after a drop in `steer_dropped` handling.\n"
             "\n"
             "• Updated `ui_protocol_transport.rs` to persist queued steers to the session ledger.\n"
             "\n"
             "• All tests pass: `12 passed`.\n"
             "\n"
             "• Changes included in commit `a6ea8505`.")

def build_09(sc):
    """Completed answer. Card #11e: the body is ONE native flowing region bound to
    `answer.markdown` (LESSONS: 'Dynamic content is a flow region'). v4 placed the
    body as measured boxes, which overlapped ('Updated' under the ui_protocol chip),
    changed font size per line and dropped 'commit'. The 'Worked for' row, the action
    icons and the timestamp stay fixed chrome."""
    rx, ry, rw, rh = sc.rows[0][1:]
    sc.add_control("worked_row", 16, 32, 374, 44, 0, bg="panel", radius=10, weight=500,
                   color="muted", lx=rx, ly=ry, lw=rw, lh=rh, event="turn.expand")
    sc.put(flow_md("answer_md", ANSWER_MD, 27, 106, 356, 492, size=17.5, line_height=38))
    sc.flows["answer_md"] = ("answer.markdown", 27, 106, 356, 492)
    sc.add_icon("icon_copy", "copy", 24, 662, 20, 20, color="muted")
    sc.add_icon("icon_thumbs", "thumbs", 52, 662, 20, 20, color="muted")
    sc.add_icon("icon_share", "share", 80, 662, 20, 20, color="muted")
    sc.add_text("t11", 10, color="muted", size=13)

MUTED = {"• 412 lines", "• 7 matches", "Working • 12s", "Ask for approval", "running 12 tests",
         "Sep 28, 9:41 PM", "Sep 28,9:41 PM", "+", "••", "Skip", "Deny", "v4-flash ▾", "V4-flash v",
         "Worked for 3m 4s ›", "Ask Octos anything"}

def build_generic(sc):
    for i, (s, x, y, w, h) in enumerate(sc.rows, 1):
        sc.put(text(f"t{i:02d}", fix(s), x, y, w, h,
                    weight=600 if i == 1 else 400,
                    color="muted" if s in MUTED else "ink"))

def build_05(sc):
    """INLINE APPROVAL. Card (hairline) with a shield icon and title
    "Run this command?", the command in a mono box, the reason as a flowing grey
    region, then three FULL-WIDTH stacked actions (black "Approve once", outlined
    "Approve for session", plain "Deny") and the key hint. Measured boxes:
    command box grey fill 19.5,160 366x83; action pills 18,366 368x67 and
    18,450 368x69; hint at 153,653."""
    # ref: shield at the card's LEFT gutter (black fill x25.5 y85 31.5×48), title
    # at its own OCR x=78 — v1 placed the shield at x92, overlapping the title.
    # ref-05-cmd-big / z-05-head: the header shield is OUTLINED WITH A CHECK inside
    # (icon `shield_check`), and the command box is NEAR-WHITE with a hairline border
    # — not the grey `box` token v6 used (pixel probe: interior #F9F9FA, page #FCFCFC).
    sc.add_icon("icon_shield", "shield_check", 26, 85, 30, 42)
    sc.add_text("t01", 0, weight=600, size=17)
    sc.put(surface("cmd_box", 20, 158, 366, 70, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=[
        code("t02", sc.t(1), 29, 185, 340, 30, weight=500, size=15)]))
    # the reason is runtime copy → a flowing region bound to one data id. The
    # reference wraps it to TWO lines ("…so Cl" / "can run"), so the region is
    # 300 wide at size 15 (ref ink h≈22).
    reason = sc.t(2) + " " + sc.t(3)
    # ref: two lines with the FIRST ending at x≈319 — so the font is larger than
    # the v8 size 17 (which fit the whole string in 289px). 21 wraps it in ~300.
    sc.put(flow_text("reason_text", reason, 21, 266, 302, 66, size=21, color="muted"))
    sc.flows["reason_text"] = ("approval.reason", 21, 266, 302, 66)
    sc.add_control("approve_once", 18, 364, 368, 70, 4, bg="black", radius=999, weight=500,
                   color="white", lx=136, ly=390, lw=200, lh=30, event="approval.approve")
    sc.add_control("approve_session", 18, 448, 368, 70, 5, bg="white", radius=999, weight=500,
                   border=1, bordercolor="hair", lx=109, ly=472, lw=240, lh=32,
                   event="approval.approve_session")
    sc.add_control("deny", 18, 532, 368, 58, 6, bg="white", radius=999, weight=400,
                   lx=170, ly=553, lw=80, lh=28, event="approval.deny")
    sc.add_text("t_hint", 7, color="muted", size=12)
    # one composition root so the subtree extracts as `approval-card` (card #18)
    sc.wrap("approval_card", 16, 74, 374, 622)

def build_06(sc):
    """USER QUESTION. Card with title "Octos needs a decision", the question as a
    flowing region (2 measured lines), three radio options (first "recommended"),
    an optional one-line note input, a black "Submit answer" pill and grey "Skip"."""
    # ref-06-hdr: the header icon is a "?" in a CIRCLE at the left gutter (x24..46),
    # and the title starts at its own OCR x=79 — v6 put a shield at x92, overlapping.
    sc.add_icon("icon_decision", "question_circle", 24, 80, 26, 28, color="muted")
    sc.add_text("t01", 0, weight=600, size=19)
    q = sc.t(1) + " " + sc.t(2)
    sc.put(flow_text("question_text", q, 37, 150, 290, 62, size=17, color="ink"))
    sc.flows["question_text"] = ("question.prompt", 37, 150, 290, 62)
    # three radio rows: a ring with a filled blue dot when chosen (option 1), a
    # bare grey ring otherwise — the reference's radio marks, not solid blobs.
    for i, (row_i, cid, event, glyph) in enumerate([
            (3, "opt_ledger", "question.select.ledger", "radio_on"),
            (5, "opt_memory", "question.select.memory", "radio_off"),
            (6, "opt_ask", "question.select.ask", "radio_off")]):
        _, x, y, w, h = sc.rows[row_i]
        sc.put(stack(cid, 34, y - 6, 340, h + 12, [
            icon(cid + "_radio", glyph, 38, y + 2, 20, 20, color="ink"),
            text(cid + "_label", sc.t(row_i), x, y, w, h, size=14,
                 weight=500 if i == 0 else 400)], event=event))
        sc.controls[cid] = (event, [int(34), int(y - 6), int(340), int(h + 12)], True)
    # the "(recommended)" suffix stays beside the first option
    _, rx, ry, rw, rh = sc.rows[4]
    sc.put(text("t_reco", sc.t(4), rx, ry, rw, rh, size=13, color="muted"))
    # note input: the reference draws a bordered rounded box around it
    # (z6 view), not bare text.
    sc.put(surface("note_box", 34, 478, 338, 50, bg="white", radius=10, border=1,
                   bordercolor="hair", kids=[
        input_node("note_input", 48, 488, 310, 30, sc.t(7), size=14)]))
    sc.inputs["note_input"] = ("question.note", [48, 488, 310, 30])
    sc.add_control("submit_answer", 24, 566, 362, 72, 8, bg="black", radius=999, weight=500,
                   color="white", lx=138, ly=591, lw=220, lh=30, event="question.submit")
    sc.add_control("skip", 24, 650, 362, 60, 9, bg="white", radius=999, weight=400,
                   color="muted", lx=180, ly=673, lw=80, lh=30, event="question.skip")
    sc.wrap("question_card", 16, 60, 374, 660)

def build_07(sc):
    """EDITED FILES. Header "Edited 3 files" with totals "+62 −5", top-right plain
    "Undo" + outlined "Review" pill, three file rows (grey dir path, black filename,
    +/− at right), and a "Show diff" chevron. File rows are a list of rows."""
    sc.add_text("t01", 0, weight=600, size=19)
    # Card #18b: the atlas draws the header totals LARGE and in two colours
    # (measured green "+62" x21..52, red "−5" x64..85, ink h≈21 => ~15pt). OCR
    # merged the two runs into ONE row ("+62 -5"), so a single width-fitted node
    # shrank to 9.5pt; author the two runs explicitly like the file rows.
    _, ttx, tty, ttw, tth = sc.rows[1]
    sc.put(text("t02_add", "+62", ttx, tty, 44, tth, weight=500, size=15, color="green"))
    sc.put(text("t02_del", "-5", ttx + 46, tty, 40, tth, weight=500, size=15, color="red"))
    # ref z7-07-undorev: "Undo" is the LABEL and the ↺ glyph sits to its RIGHT
    # (OCR "Undo 9" merged the glyph). v7 drew the icon at x231, over the label.
    _, ux, uy, uw, uh = sc.rows[2]
    sc.put(text("t_undo", "Undo", ux, uy, 62, uh, weight=500, size=14))
    sc.add_icon("icon_undo", "undo", ux + 64, uy + 2, 18, 18, color="ink")
    sc.add_control("review", 316, 78, 74, 40, 3, bg="white", radius=999, weight=500,
                   border=1, bordercolor="hair", lx=319, ly=88, lw=80, lh=24,
                   event="files.review")
    rows = [(4, 5, 6, "file_1"), (7, 8, 9, "file_2"), (10, 11, 12, "file_3")]
    # ref z7-07: the three file rows are ONE card (measured hairlines at its top
    # y178 and bottom y564) with a divider between rows (y308 / y437) — not three
    # separate rounded cards, which v8 drew.
    file_kids = []
    for dir_i, name_i, stat_i, cid in rows:
        _, dx, dy, dw, dh = sc.rows[dir_i]
        _, nx, ny, nw, nh = sc.rows[name_i]
        _, sx, sy, sw, sh = sc.rows[stat_i]
        # split the measured "+31 -4" into a green add-run and a red del-run, both
        # right-aligned in the stat column (ref-07-row1).
        stat = sc.t(stat_i).replace("−", "-")
        add, _, dele = stat.partition(" ")
        stat_x = sx + sw - 96
        file_kids.append(text(cid + "_dir", sc.t(dir_i), dx, dy, dw, dh, size=12, color="muted"))
        file_kids.append(text(cid + "_name", sc.t(name_i), dx, ny, nw, nh, size=15, weight=500))
        file_kids.append(text(cid + "_add", add, stat_x, sy, 56, sh, size=15, weight=500,
                              color="green"))
        file_kids.append(text(cid + "_del", dele, stat_x + 60, sy, 40, sh, size=15, weight=500,
                              color="red"))
        sc.controls[cid] = (f"files.open.{cid}", [16, int(dy) - 14, 374, 96], True)
    # two dividers between the three rows (measured y308 / y437)
    file_kids.append(surface("div_1", 30, 307, 346, 1, bg="hair", radius=0))
    file_kids.append(surface("div_2", 30, 436, 346, 1, bg="hair", radius=0))
    sc.put(surface("files_card", 16, 178, 374, 386, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=file_kids))
    # ref z7-07-showdiff: the chevron is at the row's RIGHT edge next to "Show diff".
    _, sx2, sy2, sw2, sh2 = sc.rows[13]
    sc.put(text("t_show", sc.t(13), sx2, sy2, sw2, sh2, weight=500, size=14))
    sc.add_icon("icon_show", "chevron_right", sx2 + sw2 + 6, sy2 + 3, 16, 16, color="muted")
    sc.wrap("edited_files_card", 16, 60, 374, 580)

def build_10(sc):
    """GOAL AND PLAN. A slim goal strip ("Goal · … · 18m") with pause/stop icons,
    above a plan card "Plan · 3 of 5" whose steps are a LIST of rows: 3 done
    (check), 1 in-progress (spinner), 1 pending (empty ring)."""
    _, gx, gy, gw, gh = sc.rows[0]
    # Card #18b: the atlas icons are ~18-20px (measured ink x315..341 / x354..371,
    # y37..55), not the 14x16 first pass; and the pause is filled (see ICONS).
    sc.put(surface("goal_strip", 16, 28, 374, 40, bg="box", radius=10, kids=[
        text("t01", sc.t(0), gx, gy, gw, gh, weight=500, size=15),
        icon("icon_pause", "pause", 306, 31, 18, 20, color="muted"),
        icon("icon_stop", "stop_filled", 342, 32, 18, 18, color="muted")]))
    sc.controls["goal_pause"] = ("goal.pause", [306, 31, 18, 20], True)
    sc.controls["goal_stop"] = ("goal.stop", [342, 32, 18, 18], True)
    sc.add_text("t02", 1, weight=600)
    steps = [(2, "done"), (3, "done"), (4, "done"), (5, "active"), (6, "pending")]
    kids = []
    for i, (row_i, state) in enumerate(steps):
        _, x, y, w, h = sc.rows[row_i]
        kids.append(text(f"step_{i}_label", sc.t(row_i), x, y, w, h,
                         weight=500 if state == "active" else 400,
                         color="ink" if state != "pending" else "muted"))
    sc.put(stack("plan_steps", 24, 200, 368, 420, kids))
    sc.flows["plan_steps"] = ("plan.steps", 24, 200, 368, 420)
    # step markers (z-10-marks): done steps are BLUE FILLED discs with a white check
    # (x40..66, r≈13); the active step is a dotted blue ring; the pending step is a
    # plain grey outline ring. v6 drew green outline checks instead.
    marks = [(2, 226, "check_circle"), (3, 311, "check_circle"), (4, 397, "check_circle"),
             (5, 482, "ring_dotted"), (6, 570, "ring")]
    for i, (row_i, my, glyph) in enumerate(marks):
        sc.add_icon(f"icon_step{i}", glyph, 40, my, 26, 26,
                    color="blue" if glyph != "ring" else "muted")
    # wrap the plan parts as `plan-card` (card #18); the goal strip was already
    # placed above and stays its own component (`goal-strip`).
    # NOTE: goal_strip was put first, so re-parent it out before wrapping.
    goal = sc.kids.pop(0) if sc.kids else None
    sc.wrap("plan_card", 16, 130, 374, 500)
    if goal is not None:
        sc.kids.insert(0, goal)

def build_11(sc):
    """REVIEW DIFF. Header "Review" + scope pill "Last turn ▾" + totals; one file
    header "ui_protocol_transport.rs  +31 −4"; the unified diff is a FLOW/LIST of
    rows (not measured boxes): a line-number gutter, 2 red removed, 4 green added,
    then a grey folded row ":412 unmodified lines"."""
    sc.add_text("t01", 0, weight=600, size=19)
    # ref-11-head: the scope pill's chevron sits at the pill's RIGHT edge, not at
    # x+10 (which landed on the "Last turn" label); and the totals are "+62" green
    # followed by "-5" red, not one small green string.
    sc.put(surface("scope_pill", 196, 30, 96, 34, bg="white", radius=999, border=1,
                   bordercolor="hair", kids=[
        text("scope_label", sc.t(1), 205, 38, 74, 22, size=14, weight=500),
        icon("scope_chev", "chevron_down", 266, 40, 16, 16, color="muted")]))
    sc.controls["scope_pill"] = ("diff.scope", [196, 30, 96, 34], True)
    # totals: "+62" green then "-5" red, split at the measured "+62 -5" row.
    _, tx, ty, tw, th = sc.rows[2]
    sc.put(text("t_add", "+62", tx, ty, 46, th, weight=500, size=15, color="green"))
    sc.put(text("t_del", "-5", tx + 48, ty, 40, th, weight=500, size=15, color="red"))
    # file header: a flat bordered row, filename in mono, +31 green / -4 red.
    _, fx, fy, fw, fh = sc.rows[3]
    sc.put(surface("file_header", 20, fy - 10, 366, 42, bg="white", radius=10, border=1,
                   bordercolor="hair", kids=[
        code("t_file", "ui_protocol_transport.rs", fx, fy, 220, fh, size=14, weight=500),
        text("t_fadd", "+31", 268, fy, 40, fh, size=14, weight=500, color="green"),
        text("t_fdel", "-4", 312, fy, 30, fh, size=14, weight=500, color="red")]))
    # the diff rows (a list of rows; the outer loop reads the flow binding).
    # Row pitch and the first row's y are the MEASURED ref bands
    # (207,256,304,357,407,457,508,558 → pitch ~50.2, start 199 for the row box).
    diff_lines = [
        ("198", "fn handle_disconnect(&mut self) {", "ctx"),
        ("199", "- self.queue.clear();", "del"),
        ("200", "- self.state = State::Disconnected;", "del"),
        ("201", "+ self.persist_queue()?;", "add"),
        ("202", "+ self.queue.mark_pending();", "add"),
        ("203", "+ self.metrics.reconnects += 1;", "add"),
        ("204", "+ self.state = State::Disconnected;", "add"),
        # Card #18b: the atlas line 205 carries a closing brace; the first pass
        # dropped it (an empty body string), so the row rendered blank.
        ("205", "}", "ctx"),
    ]
    rows = []
    for i, (num, line, kind) in enumerate(diff_lines):
        y = 199 + i * 50
        color = {"del": "red", "add": "green", "ctx": "ink"}[kind]
        gutter = code(f"ln_{i}", num, 30, y, 30, 22, color="muted")
        body = code(f"dl_{i}", line, 72, y, 320, 24, color=color)
        if kind == "del":
            # the reference tints removed rows with a soft red band (measured
            # pink rows at logical y 238-338), added rows with a soft green band.
            rows.append(surface(f"row_{i}", 24, y - 8, 358, 44, bg="redbg", radius=6,
                                kids=[gutter, body]))
        elif kind == "add":
            rows.append(surface(f"row_{i}", 24, y - 8, 358, 44, bg="greenbg", radius=6,
                                kids=[gutter, body]))
        else:
            rows.append(gutter)
            rows.append(body)
    sc.put(stack("diff_rows", 20, 180, 366, 420, rows))
    sc.flows["diff_rows"] = ("diff.rows", 20, 180, 366, 420)
    # Card #18b: the atlas folded row is "⋮ 412 unmodified lines ⋮" — OCR read the
    # leading vertical-ellipsis as a ":" (measured dots x104..106 AND x277..278
    # around the text x120..260). Author both marks explicitly.
    _, ux, uy, uw, uh = sc.rows[19]
    sc.put(surface("folded", 24, uy - 8, 358, 34, bg="box", radius=8, kids=[
        text("t_fold", "\u22ee 412 unmodified lines \u22ee", ux, uy, uw, uh,
             size=13, color="muted")]))
    # one composition root so the subtree extracts as `diff-view` (card #18), and
    # the reference encloses the whole diff in a bordered white card (measured
    # vertical hairlines at logical x25/26 and x380/381, y100..700).
    sc.wrap_card("diff_view", 20, 92, 362, 566)

def build_12(sc):
    """SETTINGS CARD. Section title "Permissions" + a grouped card with 2 rows
    (title, grey description, blue toggle at right); below, section "Model" with a
    row "Default model" and a picker "deepseek-v4-flash ▾"."""
    sc.add_text("t01", 0, weight=600, size=19)
    card_kids = [
        text("t_r1", sc.t(1), *sc.rows[1][1:], size=15, weight=500),
        # ref-12: each row's description WRAPS to two lines ("Ask before running
        # commands" / "that modify your system.") in a ~200-wide region; v6 used a
        # single_line `text` at width 280, which clipped it to one line.
        flow_text("t_d1", sc.t(2) + " " + sc.t(3), 45, 164, 210, 44, size=13,
                  color="muted"),
        text("t_r2", sc.t(4), *sc.rows[4][1:], size=15, weight=500),
        flow_text("t_d2", sc.t(5) + " " + sc.t(6), 45, 316, 210, 44, size=13,
                  color="muted"),
        # toggles: track + knob (first ON = blue, second OFF = grey)
        surface("toggle1", 312, 130, 50, 30, bg="blue", radius=999,
                kids=[surface("toggle1_knob", 336, 133, 24, 24, bg="white", radius=999)]),
        surface("toggle2", 312, 282, 50, 30, bg="hair", radius=999,
                kids=[surface("toggle2_knob", 315, 285, 24, 24, bg="white", radius=999)]),
    ]
    # ref: grouped card y97..~370 covering BOTH rows (labels at y129 / y281,
    # descriptions to y353), so h≈273 — not the too-short 232 v1 drew.
    sc.put(surface("perm_card", 28, 99, 350, 272, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=card_kids))
    # ref z7-12-divider: a full-width hairline between the two permission rows
    # (measured grey row at logical y249), inside the card.
    sc.put(surface("perm_divider", 30, 248, 346, 1, bg="hair", radius=0))
    sc.controls["toggle_default"] = ("settings.permissions.default", [320, 128, 44, 26], True)
    sc.controls["toggle_full"] = ("settings.permissions.full", [320, 280, 44, 26], True)
    sc.add_text("t_sec2", 7, weight=600, size=17)
    _, mx, my, mw, mh = sc.rows[8]
    sc.put(surface("model_card", 28, 540, 348, 72, bg="white", radius=12, border=1,
                   bordercolor="hair", kids=[
        text("t_model", sc.t(8), mx, my, mw, mh, size=15, weight=500),
        text("t_pick", fix(sc.t(9)), 205, my, 160, mh, size=14, weight=500, color="muted")]))
    sc.controls["model_picker"] = ("settings.model.select", [205, 550, 160, 32], True)
    # one composition root so the subtree extracts as `settings-group` (card #18)
    sc.wrap("settings_group", 16, 30, 374, 620)

BUILDERS = {1: build_01, 3: build_03, 4: build_04, 5: build_05, 6: build_06, 7: build_07,
            8: build_08, 9: build_09, 10: build_10, 11: build_11, 12: build_12}
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
