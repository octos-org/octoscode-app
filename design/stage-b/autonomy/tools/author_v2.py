#!/usr/bin/env python3
"""Author the 6 autonomy contracts (board 3, screens 1-6) in the design-flow's
STANDARD pattern — the same composition board 1 (conversation) uses:

    stack(id, KIT)                     <- control/surface container (Stack)
      stack(id_surface, variant=surface) <- the fill
      button(id_control)                 <- the native control
      text(id_label)                     <- copy as a Stack child
      svg(id_icon)                       <- line icons

Copy + positions are MEASURED (Apple Vision OCR on each screen's reference.png,
scaled to logical 406x776) and corrected only where the approved prompt
(source/prompt.txt) names a different string. Icon positions are pixel-scanned
from the reference (pause/play/trash/dots have no OCR ink). No data is invented.

Run:  python3 tools/author_v2.py     # writes contract.json + assets/*.svg +
                                   # service-actions.json for all 6 screens
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OCR = ROOT / "ocr"

C = {"white": 0xFFFFFFFF, "panel": 0xFFF7F7F8, "hair": 0xFFE5E5E7, "ink": 0xFF1D1D1F,
     "muted": 0xFF6E6E73, "black": 0xFF000000, "blue": 0xFF2F6FEB, "green": 0xFF1F883D,
     "greenbg": 0xFFE6F4EA, "red": 0xFFCF222E, "redbg": 0xFFFDECEC, "box": 0xFFF4F4F5}
C_HEX = {"white": "#FFFFFF", "panel": "#F7F7F8", "hair": "#E5E5E7", "ink": "#1D1D1F",
         "muted": "#6E6E73", "black": "#000000", "blue": "#2F6FEB", "green": "#1F883D",
         "greenbg": "#E6F4EA", "red": "#CF222E", "redbg": "#FDECEC", "box": "#F4F4F5"}
FONT = {400: "self:resources/ux/Inter-400.ttf", 500: "self:resources/ux/Inter-500.ttf",
        600: "self:resources/ux/Inter-600.ttf", 700: "self:resources/ux/Inter-700.ttf"}
# The kit bundles a monospace face (see conversation/tools/rebuild.sh): the host
# resolves self:resources/ux/LiberationMono-Regular.ttf.
MONO = "self:resources/ux/LiberationMono-Regular.ttf"

TITLES = {1: "Review panel", 2: "Code review run", 3: "Goal",
          4: "Loops", 5: "Monitors", 6: "Fleet"}

ICONS = {
    "chevron_down": '<path d="M6 9l6 6 6-6"/>',
    "pause": '<rect x="7.2" y="5.6" width="3.7" height="12.8" rx="1.1" fill="#6E6E73" stroke="none"/>'
             '<rect x="13.1" y="5.6" width="3.7" height="12.8" rx="1.1" fill="#6E6E73" stroke="none"/>',
    "play": '<path d="M8 5.5v13l11-6.5z"/>',
    "trash": '<path d="M4 7h16"/><path d="M9 7V4h6v3"/><path d="M6.5 7l1 13h9l1-13"/>',
    "clock": '<circle cx="12" cy="12" r="8"/><path d="M12 7v5l3.5 2"/>',
    "plus": '<path d="M12 5v14"/><path d="M5 12h14"/>',
    "file": '<path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4"/>',
    "dot_green": '<circle cx="12" cy="12" r="5" fill="#1F883D" stroke="none"/>',
    "dot_grey": '<circle cx="12" cy="12" r="5" fill="#C7C7CC" stroke="none"/>',
    "alert": '<path d="M12 4l10 17H2z"/><path d="M12 10v5"/><circle cx="12" cy="18" r="0.6" fill="#CF222E"/>',
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

ICON_REG = {}

def icon(id, name, x, y, w, h, *, color="ink"):
    ICON_REG[id] = (name, color)
    return {"t": "svg", "id": id, "x": r(x), "y": r(y), "w": r(w), "h": r(h), "src": ""}

def code(id, s, x, y, w, h, *, weight=400, color="ink", size=None):
    return text(id, s, x, y, w, h, weight=weight, color=color, size=size, font=MONO)

def flow_text(id, s, x, y, w, h, *, size=14, weight=400, color="ink", font=None,
              line_height=None):
    """A dynamic text region: one native flowing Label (default right_wrap) so
    runtime prose wraps; see conversation author_v2 flow_text docstring."""
    size = size or r(max(h / 1.5, 10.0))
    lh = line_height if line_height else size * 1.45
    return {"t": "text", "id": id, "text": s, "x": r(x), "y": r(y), "w": r(max(w, 8)),
            "h": r(h), "size": size, "line_height": r(lh),
            "weight": weight, "color": C[color], "alignx": 0,
            "font_src": font or FONT.get(weight, FONT[400])}

class Sc:
    def __init__(self, num, rows):
        self.num = num
        self.rows = rows          # [(text,x,y,w,h)] measured, logical 406x776
        self.kids = []
        self.controls = {}        # id -> (event, bounds, enabled)
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
                    color="ink", icon_name=None, event=None, enabled=1, icon_color=None):
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
            kids.append(icon(id + "_icon", icon_name, x + 10, y + (h - 18) / 2, 18, 18,
                             color=icon_color or "ink"))
        kids.append(text(id + "_label", s, lx, ly, lw, lh, weight=weight, color=color))
        li = len(kids) - 1
        self.put(stack(id, x, y, w, h, kids,
                       kit=json.dumps({"widget": "KitButton",
                                       "bindings": {"control": [1], "label": [li]}})))
        self.controls[id] = (event or id, [int(x), int(y), int(w), int(h)], bool(enabled))
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

# OCR glyph confusions corrected against the approved prompt (source/prompt.txt).
FIX = {
    "OctosCode v": "OctosCode ▾",
    "Run Cl smoke": "Run CI smoke",
    "every day • 01:00": "every day · 01:00",
    "fired 3x": "fired 3×",
    "18m • 41k tokens": "18m · 41k tokens",
    "7m • 12k tokens": "7m · 12k tokens",
    "24m • 28k tokens": "24m · 28k tokens",
    "Fleet • 3 peers": "Fleet · 3 peers",
    "Last turn -": "Last turn ▾",
    "L crates/octos-core/src/ui_protocol.rs": "📄 crates/octos-core/src/ui_protocol.rs",
    "Reviewing 3 files • 2 specialists": "Reviewing 3 files · 2 specialists",
    ": 412 unmodified lines :": "⋮ 412 unmodified lines ⋮",
}
SOURCE = {n: "design/stage-a/autonomy/atlas.png" for n in range(1, 7)}

def header(sc, title_i):
    """Every screen: app title 'OctosCode ▾' 600, then the screen title."""
    sc.add_text("t01", 0, weight=600, size=19)

# ---------------------------------------------------------------- screen 1
def build_01(sc):
    """REVIEW PANEL. Header 'Review' + scope pill 'Last turn ▾' + totals '+62 -5';
    3 file rows with per-file +add/-del; then the diff card for the first file:
    line-number gutter + marker + code rows (2 red removed, 2 green added in the
    atlas crop), closing brace, and a grey folded row '⋮ 412 unmodified lines ⋮'.
    The diff region is a flow/list (diff.rows), not measured boxes."""
    header(sc, 0)
    sc.add_text("t_title", 1, weight=600, size=19)                       # Review
    # scope pill: white pill with hairline border, label + chevron at right edge.
    _, px, py, pw, ph = sc.rows[2]                                       # Last turn ▾
    sc.put(surface("scope_pill", px - 8, py - 8, pw + 34, ph + 16, bg="white",
                   radius=999, border=1, bordercolor="hair", kids=[
        text("scope_label", sc.t(2), px, py, pw - 14, ph, size=14, weight=500),
        icon("scope_chev", "chevron_down", px + pw + 2, py + 2, 14, 16, color="muted")]))
    sc.controls["scope_pill"] = ("diff.scope", [int(px - 8), int(py - 8), int(pw + 34), int(ph + 16)], True)
    # totals: '+62' green then '-5' red, split at the measured '+62 -5' run.
    _, tx, ty, tw, th = sc.rows[3]
    sc.put(text("t_add", "+62", tx, ty, 34, th, weight=500, size=15, color="green"))
    sc.put(text("t_del", "-5", tx + 40, ty, 24, th, weight=500, size=15, color="red"))
    # file rows (OCR rows 4..8). Row 4's OCR is fused: "• <path> +31-4" (missing
    # the dot in '.rs'); split path from stats with the measured geometry.
    _, fx4, fy4, fw4, fh4 = sc.rows[4]
    sc.put(stack("file_1", 16, fy4 - 8, 374, fh4 + 16, [
        icon("file_1_icon", "file", fx4 - 2, fy4 + 1, 16, 18, color="muted"),
        code("file_1_path", "crates/octos-cli/src/api/ui_protocol_transport.rs",
             fx4 + 20, fy4, fw4 - 96, fh4, size=13, weight=400),
        text("file_1_add", "+31", fx4 + fw4 - 64, fy4, 34, fh4, size=13, weight=500, color="green"),
        text("file_1_del", "-4", fx4 + fw4 - 26, fy4, 24, fh4, size=13, weight=500, color="red")]))
    file_rows = [(5, "file_2"), (7, "file_3")]
    stats = {5: (6, "+9", "-1"), 7: (8, "+22", "-0")}
    for i, fid in file_rows:
        s, x, y, w, h = sc.rows[i]
        path = s.lstrip("•📄L ").strip()
        kids = [icon(fid + "_icon", "file", x - 2, y + 1, 16, 18, color="muted"),
                code(fid + "_path", path, x + 20, y, w - 20, h, size=13, weight=400)]
        if i in stats:
            si, a, dl = stats[i]
            _, sx2, sy2, sw2, sh2 = sc.rows[si]
            kids.append(text(fid + "_add", a, sx2, sy2, 30, sh2, size=13, weight=500, color="green"))
            kids.append(text(fid + "_del", dl, sx2 + 34, sy2, 24, sh2, size=13, weight=500, color="red"))
        sc.put(stack(fid, 16, y - 8, 374, h + 16, kids))
    # diff card title row (OCR row 9): the file whose diff is shown below.
    _, dx, dy, dw, dh = sc.rows[9]
    sc.put(stack("diff_file_header", 20, dy - 8, 366, dh + 16, [
        icon("diff_file_icon", "file", dx, dy + 1, 16, 18, color="muted"),
        code("diff_file_path", sc.t(9), dx + 20, dy, dw, dh, size=13, weight=500)]))
    # diff card for file 1 (OCR rows 9..29): gutter | marker | code, red/green bands.
    diff_lines = [
        ("128", "let msg = read_message().await?;", "ctx"),
        ("129", "if !connected {", "ctx"),
        ("130", "- queue.drop_pending();", "del"),
        ("131", "- metrics.steer_dropped += 1;", "del"),
        ("132", "+ queue.preserve_pending();", "add"),
        ("133", "+ metrics.steer_preserved += 1;", "add"),
        ("134", "+ reconnect().await?;", "add"),
        ("135", "}", "ctx"),
    ]
    _, gx, gy, _, _ = sc.rows[10]                                        # first gutter '128'
    rowh = r((sc.rows[17][2] - sc.rows[10][2]) / 7.0)                    # 128..135 span / 7
    rows = []
    for i, (num, line, kind) in enumerate(diff_lines):
        y = gy + i * rowh
        color = {"del": "red", "add": "green", "ctx": "ink"}[kind]
        gutter = code(f"ln_{i}", num, gx, y, 22, rowh - 4, color="muted")
        marker = None
        body_text = line
        if kind in ("del", "add"):
            marker = code(f"mk_{i}", line[:1], gx + 46, y, 12, rowh - 4, color=color)
            body_text = line[2:]
        body = code(f"dl_{i}", body_text, gx + 66, y, 280, rowh - 4, color=color)
        row_kids = [gutter] + ([marker] if marker else []) + [body]
        # This atlas draws diff rows on WHITE (measured #fefefe over the full row
        # band) with only the marker/code text coloured red/green — no band fill,
        # unlike conversation board 1. Emit flat rows.
        rows.extend(row_kids)
    sc.put(stack("diff_rows", 20, gy - 10, 366, rowh * 8 + 20, rows))
    sc.flows["diff_rows"] = ("diff.rows", 20, gy - 10, 366, rowh * 8 + 20)
    # folded row: '⋮ 412 unmodified lines ⋮' on the grey band.
    _, ux, uy, uw, uh = sc.rows[30]
    sc.put(surface("folded", gx - 8, uy - 8, 366, uh + 16, bg="box", radius=8, kids=[
        text("t_fold", sc.t(30), ux, uy, uw, uh, size=13, color="muted")]))
    sc.wrap_card("review_panel", 14, 92, 378, 660)

# ---------------------------------------------------------------- screen 2
def build_02(sc):
    """CODE REVIEW RUN. Header 'Code review' + black pill 'Start review'; a progress
    card ('Reviewing 3 files · 2 specialists' + muted subtitle); two finding cards:
    High (red badge) with file path + 2-line finding, Low (grey badge) likewise."""
    header(sc, 0)
    sc.add_text("t_title", 1, weight=600, size=19)                       # Code review
    _, bx, by, bw, bh = sc.rows[2]                                       # Start review
    sc.add_control("start_review", bx - 14, by - 8, bw + 28, bh + 16, 2, bg="black",
                   radius=999, color="white", weight=600, event="review.start")
    # status card (id must not contain "progress": semantics.py::classify maps any
    # stack id containing "progress" to the bound `progress` role, which demands
    # event/target bindings — this is a static status card, not a progress value).
    _, x3, y3, w3, h3 = sc.rows[3]
    _, x4, y4, w4, h4 = sc.rows[4]
    cy, ch = y3 - 16, (y4 + h4) - (y3 - 16) + 16
    sc.put(surface("run_status_card", 18, cy, 370, ch, bg="panel", radius=12, kids=[
        text("t_status", sc.t(3), x3, y3, w3, h3, size=14, weight=500),
        text("t_status_sub", sc.t(4), x4, y4, w4, h4, size=13, color="muted")]))
    # finding cards: badge pill + mono path + flowing finding text.
    def finding(badge_i, path_i, line_i, line2_i, fid, badge, bg, fg):
        _, bx2, by2, bw2, bh2 = sc.rows[badge_i]
        _, pxx, pyy, pw2, ph2 = sc.rows[path_i]
        _, lx, ly, lw, lh = sc.rows[line_i]
        _, l2x, l2y, l2w, l2h = sc.rows[line2_i]
        cy2, ch2 = by2 - 14, (l2y + l2h) - (by2 - 14) + 18
        sc.put(surface(fid, 18, cy2, 370, ch2, bg="white", radius=12, border=1,
                       bordercolor="hair", kids=[
            surface(fid + "_badge", bx2 - 10, by2 - 4, bw2 + 20, bh2 + 8, bg=bg,
                    radius=999, kids=[
                text(fid + "_badge_label", badge, bx2, by2, bw2, bh2, size=13,
                     weight=600, color=fg)]),
            code(fid + "_path", sc.t(path_i), pxx, pyy, pw2, ph2, size=12, color="muted"),
            flow_text(fid + "_text", sc.t(line_i) + " " + sc.t(line2_i),
                      lx, ly, max(lw, l2w), (l2y + l2h) - ly, size=14)]))
    finding(5, 6, 7, 8, "finding_high", "High", "redbg", "red")
    finding(9, 10, 11, 12, "finding_low", "Low", "box", "muted")
    sc.wrap("review_run", 0, 0, 406, 776)

# ---------------------------------------------------------------- screen 3
def build_03(sc):
    """GOAL. Header 'Goal'; one goal card: title 'Fix steer queue on reconnect',
    green 'Active' badge, 'Token budget' + right-aligned '41k of 100k', a progress
    bar (blue fill over hairline track), 'Elapsed' + right-aligned '18m', black
    'Pause' + white 'Stop' buttons side by side, and a 'Clear goal' link below."""
    header(sc, 0)
    sc.add_text("t_title", 1, weight=600, size=19)                       # Goal
    _, gx, gy, gw, gh = sc.rows[2]                                       # goal title
    _, ax, ay, aw, ah = sc.rows[3]                                       # Active
    _, tx, ty, tw, th = sc.rows[4]                                       # Token budget
    _, vx, vy, vw, vh = sc.rows[5]                                       # 41k of 100k
    _, ex, ey, ew, eh = sc.rows[6]                                       # Elapsed
    _, mvx, mvy, mvw, mvh = sc.rows[7]                                   # 18m
    _, px, py, pw, ph = sc.rows[8]                                       # Pause
    _, sx2, sy2, sw2, sh2 = sc.rows[9]                                   # Stop
    # progress bar: pixel-measured track y (between budget value and Elapsed rows)
    bar_y = r(vy + vh + 22)
    bar_h = 8
    kids = [
        text("t_goal", sc.t(2), gx, gy, gw, gh, size=15, weight=600),
        surface("goal_badge", ax - 10, ay - 4, aw + 20, ah + 8, bg="greenbg",
                radius=999, kids=[
            text("goal_badge_label", sc.t(3), ax, ay, aw, ah, size=13, weight=600,
                 color="green")]),
        text("t_budget", sc.t(4), tx, ty, tw, th, size=14, color="muted"),
        text("t_budget_val", sc.t(5), vx, vy, vw, vh, size=14, weight=500),
        surface("bar_track", gx, bar_y, 330, bar_h, bg="hair", radius=999),
        surface("bar_fill", gx, bar_y, r(330 * 0.41), bar_h, bg="blue", radius=999),
        text("t_elapsed", sc.t(6), ex, ey, ew, eh, size=14, color="muted"),
        text("t_elapsed_val", sc.t(7), mvx, mvy, mvw, mvh, size=14, weight=500),
    ]
    # buttons: Pause black pill, Stop white pill with hairline border; measured
    # button band (pixel scan): [35,202] and [226,392] -> logical /1.172.
    band_y = r(py - 16)
    kids.append(stack("pause_btn", 30, band_y, 143, ph + 32, [
        surface("pause_btn_surface", 30, band_y, 143, ph + 32, bg="black", radius=999),
        {"t": "button", "id": "pause_btn_control", "x": 30.0, "y": band_y, "w": 143.0,
         "h": r(ph + 32), "enabled": 1},
        text("pause_btn_label", sc.t(8), px, py, pw, ph, size=14, weight=600, color="white")],
        kit=json.dumps({"widget": "KitButton", "bindings": {"control": [1], "label": [2]}})))
    kids.append(stack("stop_btn", 193, band_y, 142, ph + 32, [
        surface("stop_btn_surface", 193, band_y, 142, ph + 32, bg="white", radius=999,
                border=1, bordercolor="hair"),
        {"t": "button", "id": "stop_btn_control", "x": 193.0, "y": band_y, "w": 142.0,
         "h": r(ph + 32), "enabled": 1},
        text("stop_btn_label", sc.t(9), sx2, sy2, sw2, sh2, size=14, weight=600, color="ink")],
        kit=json.dumps({"widget": "KitButton", "bindings": {"control": [1], "label": [2]}})))
    sc.controls["pause_btn"] = ("goal.pause", [30, int(band_y), 143, int(ph + 32)], True)
    sc.controls["stop_btn"] = ("goal.stop", [193, int(band_y), 142, int(ph + 32)], True)
    card_top = r(gy - 18)
    card_h = r((band_y + ph + 32) - card_top + 18)
    sc.put(surface("goal_card", 16, card_top, 374, card_h, bg="white", radius=12,
                   border=1, bordercolor="hair", kids=kids))
    sc.flows["goal_card"] = ("goal.card", 16, card_top, 374, card_h)
    _, cx, cy2, cw, ch2 = sc.rows[10]                                    # Clear goal
    sc.add_control("clear_goal", cx - 12, cy2 - 6, cw + 24, ch2 + 12, 10, bg="white",
                   radius=8, color="red", weight=500, event="goal.clear")
    sc.wrap("goal_screen", 0, 0, 406, 776)

# ---------------------------------------------------------------- screen 4
def build_04(sc):
    """LOOPS. Header 'Loops' + blue '+ New loop'; three loop rows, each: status dot
    (green active / grey paused), name, muted cadence, and right-side icons —
    active rows carry pause-circled + play-circled + trash; the paused row
    ('Nightly review') carries only play-circled + trash (no pause icon)."""
    header(sc, 0)
    sc.add_text("t_title", 1, weight=600, size=19)                       # Loops
    _, nx, ny, nw, nh = sc.rows[10]                                      # + New loop
    sc.add_control("new_loop", nx - 12, ny - 6, nw + 24, nh + 12, 10, bg="white",
                   radius=8, color="blue", weight=600, icon_name="plus",
                   icon_color="blue", event="loop.new")
    rows = [
        (2, 3, "loop_1", "dot_green", True),    # Run CI smoke, every 15 min
        (4, 5, "loop_2", "dot_green", True),    # Sync main, every 30 min
        (6, 7, "loop_3", "dot_grey", False),    # Nightly review, paused
    ]
    # icon x positions from the pixel scan (atlas px -> logical /1.252).
    ic = {"pause": 257.2, "play": 306.9, "trash": 356.2}
    for name_i, cad_i, rid, dot, active in rows:
        _, x, y, w, h = sc.rows[name_i]
        _, cx2, cy2, cw2, ch2 = sc.rows[cad_i]
        kids = [icon(rid + "_dot", dot, 204.5, y - 4, 20, 26),
                text(rid + "_name", sc.t(name_i), x, y, w, h, size=15, weight=500),
                text(rid + "_cad", sc.t(cad_i), cx2, cy2, cw2, ch2, size=13, color="muted")]
        iy = r(cy2 + ch2 + 14)
        if active:
            kids.append(icon(rid + "_pause", "pause", ic["pause"], iy, 18, 20, color="muted"))
        kids.append(icon(rid + "_play", "play", ic["play"], iy, 18, 20, color="muted"))
        kids.append(icon(rid + "_trash", "trash", ic["trash"], iy, 18, 20, color="muted"))
        top = r(y - 16)
        sc.put(surface(rid, 16, top, 374, r((iy + 26) - top), bg="white", radius=12,
                       border=1, bordercolor="hair", kids=kids))
        sc.controls[rid + "_pause" if active else rid + "_play"] = (
            "loop.toggle", [16, int(top), 374, int((iy + 26) - top)], True)
    sc.wrap("loops_screen", 0, 0, 406, 776)

# ---------------------------------------------------------------- screen 5
def build_05(sc):
    """MONITORS. Header 'Monitors'; two monitor cards: mono command, muted state
    ('fired 3×' / 'no change'), clock icon + '30s' interval, and pause + trash
    icons; footer with info line 'Monitors fire when the output changes'."""
    header(sc, 0)
    sc.add_text("t_title", 1, weight=600, size=19)                       # Monitors
    cards = [(2, 3, 4, "mon_1"), (5, 6, 7, "mon_2")]
    for cmd_i, state_i, int_i, cid in cards:
        _, x, y, w, h = sc.rows[cmd_i]
        _, sx2, sy2, sw2, sh2 = sc.rows[state_i]
        _, ix, iy, iw, ih = sc.rows[int_i]
        kids = [code(cid + "_cmd", sc.t(cmd_i), x, y, w, h, size=14, weight=500),
                text(cid + "_state", sc.t(state_i), sx2, sy2, sw2, sh2, size=13, color="muted"),
                icon(cid + "_clock", "clock", ix - 24, iy - 1, 18, 18, color="muted"),
                text(cid + "_int", sc.t(int_i), ix, iy, iw, ih, size=13, color="muted"),
                icon(cid + "_pause", "pause", 296.0, sy2 - 6, 18, 20, color="muted"),
                icon(cid + "_trash", "trash", 348.7, sy2 - 6, 18, 20, color="muted")]
        top = r(y - 16)
        sc.put(surface(cid, 16, top, 374, r((sy2 + sh2 + 18) - top), bg="white",
                       radius=12, border=1, bordercolor="hair", kids=kids))
        sc.controls[cid] = ("monitor.toggle", [16, int(top), 374, int((sy2 + sh2 + 18) - top)], True)
    # footer info line
    _, fx, fy, fw, fh = sc.rows[8]
    sc.put(stack("monitors_footer", 16, fy - 10, 374, fh + 20, [
        icon("monitors_footer_icon", "clock", fx - 28, fy, 20, 20, color="muted"),
        text("monitors_footer_label", sc.t(8), fx, fy, fw, fh, size=13, color="muted")]))
    sc.wrap("monitors_screen", 0, 0, 406, 776)

# ---------------------------------------------------------------- screen 6
def build_06(sc):
    """FLEET. Header 'Fleet · 3 peers'; goal line 'Fix steer queue'; three peer rows:
    status word (Running green / Blocked red / Done muted), peer name, muted
    'elapsed · tokens' line, and a blue 'Steer' link at the row's right edge."""
    header(sc, 0)
    sc.add_text("t_title", 1, weight=600, size=19)                       # Fleet · 3 peers
    _, gx, gy, gw, gh = sc.rows[2]
    sc.put(stack("fleet_goal", 16, gy - 12, 374, gh + 24, [
        icon("fleet_goal_icon", "dot_green", gx - 2, gy + 1, 18, 18),
        text("fleet_goal_label", sc.t(2), gx + 24, gy, gw, gh, size=14, weight=500)]))
    peers = [(3, 4, 5, 6, "peer_1", "green"),      # Running, tests, 18m · 41k
             (7, 8, 9, 10, "peer_2", "red"),       # Blocked, docs, 7m · 12k
             (11, 12, 13, 14, "peer_3", "muted")]  # Done, review, 24m · 28k
    for st_i, name_i, meta_i, steer_i, pid, stc in peers:
        _, sx2, sy2, sw2, sh2 = sc.rows[st_i]
        _, nx, ny, nw, nh = sc.rows[name_i]
        _, mx, my, mw, mh = sc.rows[meta_i]
        _, tx, ty, tw, th = sc.rows[steer_i]
        top = r(sy2 - 16)
        bottom = r(my + mh + 16)
        sc.put(surface(pid, 16, top, 374, bottom - top, bg="white", radius=12,
                       border=1, bordercolor="hair", kids=[
            text(pid + "_status", sc.t(st_i), sx2, sy2, sw2, sh2, size=14, weight=600, color=stc),
            text(pid + "_name", sc.t(name_i), nx, ny, nw, nh, size=14, weight=500),
            text(pid + "_meta", sc.t(meta_i), mx, my, mw, mh, size=12, color="muted"),
            text(pid + "_steer", sc.t(steer_i), tx, ty, tw, th, size=14, weight=600, color="blue")]))
        sc.controls[pid + "_steer"] = ("peer.steer", [int(tx - 8), int(ty - 4),
                                       int(tw + 16), int(th + 8)], True)
    sc.wrap("fleet_screen", 0, 0, 406, 776)

BUILDERS = {1: build_01, 2: build_02, 3: build_03, 4: build_04, 5: build_05, 6: build_06}

def build(num):
    sc = Sc(num, load_ocr(num))
    BUILDERS[num](sc)
    kids = sc.kids
    doc = {"schema_version": 1, "id": f"autonomy-{num:02d}", "app": "octoscode",
           "number": num, "title": TITLES[num],
           "structure": "Native component reconstructed from the approved autonomy atlas",
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                       "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
           "content_source": "Approved stage-a autonomy atlas + measured Apple Vision OCR bounds",
           "graphics": {},
           "tree": stack("page", 0, 0, 406, 776, kids, variant="surface", bg=C["white"])}
    return doc, sc

if __name__ == "__main__":
    for n in range(1, 7):
        ICON_REG.clear()
        doc, sc = build(n)
        d = ROOT / "cards" / f"autonomy-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        for iid, (name, color) in ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(svg(name, C_HEX.get(color, "#1D1D1F")))
        controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                    for cid, (ev, b, en) in sc.controls.items()}
        for cid, (ev, b) in sc.inputs.items():
            controls[cid] = {"event": ev, "source_bounds": b, "enabled": True}
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": controls, "source": SOURCE[n]}, indent=2) + "\n")
        print(f"wrote autonomy-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(ICON_REG)} icons, {len(controls)} controls")
