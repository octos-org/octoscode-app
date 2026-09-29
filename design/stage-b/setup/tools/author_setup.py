#!/usr/bin/env python3
"""Author the six #28c setup contracts (setup-01..06) + text-tree stubs (07..12).

Reuses the conversation DSL (author_v2.py: stack/surface/text/icon/input_node,
Scene.add_control/add_input) so the trees follow the same STANDARD pattern proven
at design.rs's Stack arm (conversation card #11b). Text copy + positions are
MEASURED (Apple Vision OCR, `ocr/setup-NN.ocr.json`, 812x1552 -> logical 406x776),
corrected only where the approved stage-a prompt names a different string.

Scenes 7..12 belong to other cards; they exist so the flow's 8-12-scene
validation passes and semantic/compile can run over the whole manifest. They get
build_generic-style text-only trees (an empty tree would fail preflight against
their real reference text).
"""
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
AV_PATH = HERE.parent.parent / "conversation" / "tools" / "author_v2.py"
spec = importlib.util.spec_from_file_location("author_v2", AV_PATH)
av = importlib.util.module_from_spec(spec)
spec.loader.exec_module(av)

ROOT = HERE.parent
OCR = ROOT / "ocr"

TITLES = {
    1: "Connect", 2: "Connect failed", 3: "Onboarding", 4: "Workspace picker",
    5: "Session settings", 6: "General settings", 7: "Model settings",
    8: "Command palette", 9: "Context panel", 10: "Skills",
    11: "Error screen", 12: "Loading and reconnecting",
}

# OCR quirks corrected to the approved stage-a copy (prompt + atlas review).
FIX = {
    "octos-der": "octos-dev",
    "octos >": "octos",
    "deepseek-v4-flash ×": "deepseek-v4-flash",
    "• Live": "Live",
    "U. ULNLAAL OLITINOO": None,          # gutter shadow above screen 6, not UI
    "Last tried 9:41 PM • Retry": "Last tried 9:41 PM · Retry",
    "Enabled • Network off • Workspace write": "Enabled · Network off · Workspace write",
    "Browse folders...": "Browse folders…",
    "OctosCode v": "OctosCode ▾",
}
DROP = {None}

EYE_SVG = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
           'stroke="#6E6E73" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'
           '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/></svg>\n')

FOLDER_SVG = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
              'stroke="#1D1D1F" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'
              '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>\n')


def svg_node(sc, id, content, x, y, w, h):
    """A custom-line-icon svg node whose asset content is authored here."""
    sc.icons[id] = content
    return {"t": "svg", "id": id, "x": av.r(x), "y": av.r(y), "w": av.r(w), "h": av.r(h), "src": ""}


def load_rows(num):
    d = json.loads((OCR / f"setup-{num:02d}.ocr.json").read_text())
    sx, sy = 406 / d["width"], 776 / d["height"]
    rows = []
    for o in d["observations"]:
        s = FIX.get(o["text"], o["text"])
        if s in DROP:
            continue
        x, y, w, h = o["bounds"]
        rows.append((s, x * sx, y * sy, w * sx, h * sy))
    return rows


def titled_box(sc, id, box_i, *, icon_name=None, eye=False, chevron=False, value=None,
               bordercolor="hair"):
    """A panel input: bordered surface + input showing the measured value (dark ink)."""
    s, x, y, w, h = sc.rows[box_i]
    bx, by, bw, bh = 46, y - 12, 314, h + 24
    field = av.input_node(id + "_field", bx + 14, by + (bh - h) / 2, bw - 28, h, s)
    if value is not None:
        field["text"] = value
        field["placeholder"] = ""
        field["color"] = av.C["ink"]
    kids = [field]
    if chevron:
        kids.append(av.icon(id + "_chev", "chevron_down", bx + bw - 30, by + (bh - 20) / 2, 20, 20, color="muted"))
    if eye:
        kids.append({"t": "svg", "id": id + "_eye", "x": av.r(bx + bw - 32),
                     "y": av.r(by + (bh - 20) / 2), "w": av.r(20), "h": av.r(20), "src": ""})
        sc.icons[id + "_eye"] = EYE_SVG
    sc.put(av.surface(id, bx, by, bw, bh, bg="panel", radius=8, border=1,
                      bordercolor=bordercolor, kids=kids))
    sc.inputs[id + "_field"] = ("input." + id, [int(bx), int(by), int(bw), int(bh)])


def black_pill(sc, id, label_i, *, cx=None, event=None, bg="black", color="white"):
    s, x, y, w, h = sc.rows[label_i]
    cw, ch = w + 42, h + 20
    cx = x + w / 2 if cx is None else cx
    sc.add_control(id, cx - cw / 2, y - 10, cw, ch, label_i, bg=bg, radius=999,
                   color=color, weight=600, event=event or id)


def build_01(sc):
    sc.add_text("t_title", 0, weight=600, size=20)
    sc.add_text("t_server", 1, size=14, color="muted")
    titled_box(sc, "server", 2, value=sc.rows[2][0])
    sc.add_text("t_token", 3, size=14, color="muted")
    titled_box(sc, "token", 4, eye=False, value="•" * 19)
    sc.add_text("t_hint", 5, size=13, color="muted")
    black_pill(sc, "connect", 6)
    s, x, y, w, h = sc.rows[7]
    sc.put(av.text("t_local", s, x, y, w, h, size=13, color="blue", weight=500))
    sc.controls["local_solo"] = ("connect.use_local_solo", [int(x) - 8, int(y) - 6, int(w) + 16, int(h) + 12], True)


def build_02(sc):
    sc.add_text("t_title", 0, weight=600, size=20)
    sc.add_text("t_server", 1, size=14, color="muted")
    titled_box(sc, "server", 2, value=sc.rows[2][0])
    sc.add_text("t_token", 3, size=14, color="muted")
    titled_box(sc, "token", 4, eye=True, value="•" * 19, bordercolor="red")
    s, x, y, w, h = sc.rows[5]
    sc.put(av.surface("error_band", 46, y - 8, 314, h + 16, bg="redbg", radius=8,
                      kids=[av.text("t_error", s, x, y, w, h, size=13, color="red", weight=500)]))
    s, x, y, w, h = sc.rows[6]
    pre, _, post = s.partition("· Retry")
    sc.put(av.text("t_last", pre.rstrip(), x, y, w, h, size=13, color="muted"))
    rx = x + 150
    sc.add_control("retry", rx, y - 7, 62, h + 14, 6, bg="white", radius=8, border=1,
                   bordercolor="hair", event="connect.retry",
                   lx=rx + 10, ly=y, lw=42, lh=h)
    sc.kids[-1]["c"][-1]["text"] = "Retry"
    black_pill(sc, "connect", 7)
    s, x, y, w, h = sc.rows[8]
    sc.put(av.text("t_local", s, x, y, w, h, size=13, color="blue", weight=500))
    sc.controls["local_solo"] = ("connect.use_local_solo", [int(x) - 8, int(y) - 6, int(w) + 16, int(h) + 12], True)


def build_03(sc):
    sc.add_text("t_title", 0, weight=600, size=22)
    sc.add_text("t_pname", 1, size=14, color="muted")
    titled_box(sc, "profile", 2, value=sc.rows[2][0])
    sc.add_text("t_provider", 3, size=14, color="muted")
    for k, (name_i, rid, on) in enumerate([(5, "deepseek", True), (6, "kimi", False), (7, "glm", False)]):
        s, x, y, w, h = sc.rows[name_i]
        sc.put(av.surface(f"provider_{rid}", 46, y - 10, 314, h + 20,
                          bg="sel" if on else "white", radius=10,
                          kids=[av.icon(f"radio_{rid}", "radio_on" if on else "radio_off",
                                        62, y + (h - 20) / 2, 20, 20,
                                        color="blue" if on else "muted"),
                                av.text(f"t_prov_{rid}", s, 92, y, 250, h, size=14, weight=500)]))
        sc.controls[f"provider_{rid}"] = (f"onboarding.provider.{rid}", [46, int(y) - 10, 314, int(h) + 20], True)
    sc.add_text("t_apikey", 4, size=14, color="muted")
    titled_box(sc, "apikey", 8, eye=True)
    sc.add_text("t_keyhint", 9, size=13, color="muted")
    black_pill(sc, "create_profile", 10)


def build_04(sc):
    sc.add_text("t_title", 0, weight=600, size=20)
    ls, lx, ly, lw, lh = sc.rows[1]
    vs, vx, vy, vw, vh = sc.rows[2]
    fld = av.input_node("folder_field", 92, vy, 200, vh, "")
    fld["text"] = vs
    fld["color"] = av.C["muted"]
    box_h = (vy + vh) - ly + 26
    sc.put(av.surface("folder", 46, ly - 14, 314, box_h, bg="panel", radius=8,
                      border=1, bordercolor="hair",
                      kids=[svg_node(sc, "folder_icon", FOLDER_SVG, 62, ly + (lh - 20) / 2, 20, 20),
                            av.text("t_sfolder", ls, 92, ly, 200, lh, size=14, weight=500, color="ink"),
                            fld,
                            av.icon("folder_chev", "chevron_right", 334, ly - 14 + box_h / 2 - 9, 18, 18, color="muted")]))
    sc.inputs["folder_field"] = ("input.folder", [46, int(ly) - 14, 314, int(box_h)])
    pairs = [(4, 5), (6, 7), (8, 9), (10, 11)]
    g_kids = []
    g_top = sc.rows[4][2] - 14
    for k, (ni, pi) in enumerate(pairs):
        ns, nx, ny, nw, nh = sc.rows[ni]
        ps, px, py, pw, ph = sc.rows[pi]
        if k:
            prev = sc.rows[pairs[k - 1][1]]
            g_kids.append(av.surface(f"hair_ws{k}", 58, (prev[2] + prev[4] + ny) / 2, 290, 1,
                                     bg="hair", radius=0))
        g_kids.append(svg_node(sc, f"ws_icon{k}", FOLDER_SVG, 60, ny + (nh - 18) / 2, 18, 18))
        g_kids.append(av.text(f"t_ws{k}_name", ns, nx, ny, nw, nh, size=14, weight=500))
        g_kids.append(av.text(f"t_ws{k}_path", ps, nx, py, 240, ph, size=12, color="muted"))
        sc.controls[f"ws_row{k}"] = ("workspace.open", [46, int(ny) - 14, 314, int(nh + ph + 24)], True)
    last = sc.rows[pairs[-1][1]]
    g_bot = last[2] + last[4] + 14
    sc.put(av.surface("recent_group", 46, g_top, 314, g_bot - g_top, bg="white", radius=10,
                      border=1, bordercolor="hair", kids=g_kids))
    black_pill(sc, "browse", 12, bg="white", color="ink")
    sc.kids[-1]["c"][0]["border"] = 1
    sc.kids[-1]["c"][0]["bordercolor"] = av.C["ink"]
    ns, nx, ny, nw, nh = sc.rows[13]
    sc.put(av.text("t_newfolder", ns, nx, ny, nw, nh, size=14, weight=500, color="ink"))
    sc.controls["new_folder"] = ("workspace.new_folder", [int(nx) - 8, int(ny) - 6, int(nw) + 16, int(nh) + 12], True)


def build_05(sc):
    sc.add_text("t_title", 0, weight=600, size=20)
    xs, ys, ws, hs = sc.rows[1][1:]
    sc.put(av.text("t_close", "✕", xs, ys, ws, hs, size=16, weight=500, color="ink"))
    sc.controls["close"] = ("settings.close", [int(xs) - 8, int(ys) - 8, int(ws) + 16, int(hs) + 16], True)
    sc.add_text("t_model", 2, size=14, color="ink")
    titled_box(sc, "model", 3, chevron=True, value=sc.rows[3][0])
    sc.add_text("t_saved", 4, size=13, color="muted")
    sc.add_text("t_perm", 5, size=14, color="ink")
    seg_y = sc.rows[6][2] - 11
    kids = [av.surface("seg_sel", sc.rows[6][1] - 10, seg_y + 4, sc.rows[6][3] + 20, 30, bg="white", radius=7)]
    for k, i in enumerate([6, 7, 8]):
        s, x, y, w, h = sc.rows[i]
        kids.append(av.text(f"t_seg{k}", s, x, y, w, h, size=13,
                            weight=600 if k == 0 else 400,
                            color="ink" if k == 0 else "muted"))
        sc.controls[f"perm_seg{k}"] = (f"settings.permissions.{s.lower().replace(' ', '_')}",
                                        [int(x) - 8, int(y) - 8, int(w) + 16, int(h) + 16], True)
    d1 = (sc.rows[6][1] + sc.rows[6][3] + sc.rows[7][1]) / 2
    d2 = (sc.rows[7][1] + sc.rows[7][3] + sc.rows[8][1]) / 2
    kids += [av.surface("segdiv1", d1, seg_y + 4, 1, 30, bg="hair", radius=0),
             av.surface("segdiv2", d2, seg_y + 4, 1, 30, bg="hair", radius=0)]
    sc.put(av.surface("segments", 46, seg_y, 314, 38, bg="sel", radius=8, border=1,
                      bordercolor="hair", kids=kids))
    sc.add_text("t_sandbox", 9, size=14, color="ink")
    s, x, y, w, h = sc.rows[10]
    sc.put(av.surface("sandbox_box", 46, y - 10, 314, h + 20, bg="white", radius=8, border=1,
                      bordercolor="hair",
                      kids=[av.text("t_sandbox_v", s, x, y, w, h, size=13, color="ink")]))


def build_06(sc):
    sc.add_text("t_title", 0, weight=600, size=22)
    rows = [(1, 2), (3, 4), (5, 6)]
    gx, gy, gw = 28, 142, 350
    card = []
    for k, (li, vi) in enumerate(rows):
        ls, lx, ly, lw, lh = sc.rows[li]
        vs, vx, vy, vw, vh = sc.rows[vi]
        card.append(av.text(f"t_g1_r{k}", ls, lx, ly, lw, lh, size=14, weight=500))
        if k == 0:
            card.append(av.surface("live_dot", vx - 16, vy + (vh - 8) / 2, 8, 8, bg="green", radius=999))
            card.append(av.text("t_g1_v0", vs, vx, vy, vw, vh, size=13, color="green"))
            card.append(av.icon("chev_g1_r0", "chevron_right", vx + vw + 6, vy + (vh - 16) / 2, 16, 16, color="muted"))
        else:
            card.append(av.text(f"t_g1_v{k}", vs, vx, vy, vw, vh, size=13, color="muted"))
            card.append(av.icon(f"chev_g1_r{k}", "chevron_right", vx + vw + 6, vy + (vh - 16) / 2, 16, 16, color="muted"))
        if k:
            pli = rows[k - 1][0]
            pbottom = sc.rows[pli][2] + sc.rows[pli][4]
            card.append(av.surface(f"hair_g1_{k}", gx + 16, (pbottom + ly) / 2, gw - 32, 1,
                                   bg="hair", radius=0))
        sc.controls[f"g1_r{k}"] = ("settings.open", [gx, int(ly) - 12, gw, int(lh) + 24], True)
    sc.put(av.surface("group1", gx, gy, gw, 196, bg="white", radius=12, border=1,
                      bordercolor="hair", kids=card))
    ns, nx, ny, nw, nh = sc.rows[7]
    sc.put(av.surface("group2", gx, 370, gw, 62, bg="white", radius=12, border=1,
                      bordercolor="hair",
                      kids=[av.text("t_notif", ns, nx, ny, nw, nh, size=14, weight=500),
                            av.surface("toggle1", 268, ny - 6, 50, 30, bg="blue", radius=999,
                                       kids=[av.surface("toggle1_knob", 294, ny - 3, 24, 24, bg="white", radius=999)])]))
    sc.controls["notif_toggle"] = ("settings.notifications", [gx, 370, gw, 62], True)
    cs, cx, cy, cw, ch = sc.rows[8]
    sc.put(av.surface("group3", gx, 462, gw, 76, bg="white", radius=12, border=1,
                      bordercolor="hair",
                      kids=[av.text("t_diag", cs, cx, cy, cw, ch, size=14, weight=500),
                            av.icon("copy_diag", "copy", gx + gw - 30, cy + (ch - 18) / 2, 18, 18, color="muted")]))
    sc.controls["copy_diag"] = ("settings.copy_diagnostics", [gx, 462, gw, 56], True)
    ds, dx, dy, dw, dh = sc.rows[9]
    sc.put(av.surface("disconnect", gx, dy - 14, gw, dh + 28, bg="white", radius=12,
                      border=1, bordercolor="hair",
                      kids=[av.text("t_disconnect", ds, dx, dy, dw, dh, size=14,
                                    weight=600, color="red")]))
    sc.controls["disconnect"] = ("settings.disconnect", [gx, int(dy) - 14, gw, int(dh) + 28], True)


BUILDERS = {1: build_01, 2: build_02, 3: build_03, 4: build_04, 5: build_05, 6: build_06}


def build_generic(sc):
    for i, (s, x, y, w, h) in enumerate(sc.rows):
        sc.put(av.text(f"t{i:02d}", s, x, y, w, h, weight=600 if i == 0 else 400,
                       size=18 if i == 0 else 14))


def build(num):
    sc = av.Scene(num, load_rows(num))
    (BUILDERS.get(num, build_generic))(sc)
    if num <= 6:
        # every target screen draws ONE enclosing rounded hairline card
        kids = sc.kids
        sc.kids = []
        sc.put(av.surface("outer_card", 10, 10, 386, 756, bg="white", radius=14,
                          border=1, bordercolor="hair", kids=kids))
    doc = {"schema_version": 1, "id": f"setup-{num:02d}", "app": "octoscode", "number": num,
           "title": TITLES[num],
           "structure": ("Native component reconstructed from the approved setup atlas (board 2, #28c)"
                         if num <= 6 else
                         "Text-tree stub: other card's screen; exists only for flow scene validation"),
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                       "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
           "content_source": "Approved stage-a atlas + measured Apple Vision OCR bounds",
           "graphics": {},
           "tree": av.stack("page", 0, 0, 406, 776, sc.kids, variant="surface", bg=av.C["white"])}
    return doc, sc


def stub_actions(num):
    return {"frame_id": num, "controls": {},
            "source": f"stub: setup-{num:02d} is another card's screen; no #28c mapping"}


if __name__ == "__main__":
    for n in range(1, 13):
        av.ICON_REG.clear()
        av.Scene.icons_default = None
        doc, sc = build(n)
        d = ROOT / "cards" / f"setup-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        extra = dict(sc.icons) if hasattr(sc, "icons") else {}
        for iid, (name, color) in av.ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(av.svg(name, av.C_HEX.get(color, "#1D1D1F")))
        for iid, content in extra.items():
            (d / "assets" / f"{iid}.svg").write_text(content)
        if n <= 6:
            controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                        for cid, (ev, b, en) in sc.controls.items()}
            for cid, (ev, b) in sc.inputs.items():
                controls[cid] = {"event": ev, "source_bounds": b, "enabled": True}
            actions = {"frame_id": n, "controls": controls,
                       "source": "Approved setup atlas (board 2) + measured OCR"}
        else:
            actions = stub_actions(n)
        (d / "service-actions.json").write_text(json.dumps(actions, indent=2) + "\n")
        print(f"wrote setup-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(av.ICON_REG) + len(extra)} icons, {len(actions['controls'])} controls")
