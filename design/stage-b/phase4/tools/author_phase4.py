#!/usr/bin/env python3
"""Author the #D1 phase4 contracts (p4-01..09) from the approved phase4-new atlas.

Same pattern as setup/tools/author_setup.py: importlib-load the conversation DSL
(author_v2.py: stack/surface/text/input_node + Scene), rows are the MEASURED
Apple Vision OCR bounds (cards/p4-NN/reference.ocr.json, 812x1552 -> logical
406x776), text copy corrected to the approved stage-a prompt. Gutter captions
("N. Title") and viewfinder-bracket OCR misreads ("7", "L") are not UI.
"""
import importlib.util, json
from pathlib import Path

HERE = Path(__file__).resolve().parent
AV_PATH = HERE.parent.parent / "conversation" / "tools" / "author_v2.py"
spec = importlib.util.spec_from_file_location("author_v2", AV_PATH)
av = importlib.util.module_from_spec(spec)
spec.loader.exec_module(av)
av.C["d2"] = 0xFFD2D2D7
av.C["accent"] = 0xFF2F6FEB   # the atlas link/toggle blue (atlas-prompt.md palette)
av.C["cal_red"] = 0xFFFDECEC  # link-problem callout background (#FDECEC)
av.C["red_tx"] = 0xFFCF222E   # callout / destructive text (#CF222E)

ROOT = HERE.parent
TITLES = {1: "Pair this device", 2: "Pairing", 3: "Link problem", 4: "Can't pair",
          5: "Paired", 6: "Provider editor", 7: "Provider rejected",
          8: "Choose a folder", 9: "Folder refused"}


def load_rows(num):
    d = json.loads((ROOT / "cards" / f"p4-{num:02d}" / "reference.ocr.json").read_text())
    rows = [(o["text"].strip(), o["bounds"][0] / 2, o["bounds"][1] / 2,
             o["bounds"][2] / 2, o["bounds"][3] / 2)
            for o in sorted(d["observations"], key=lambda o: (o["bounds"][1], o["bounds"][0]))]
    return rows


def find(sc, prefix):
    for i, (s, *_) in enumerate(sc.rows):
        if s.lower().startswith(prefix.lower()):
            return i
    raise KeyError(prefix)


def svg_node(sc, id, content, x, y, w, h):
    sc.icons[id] = content
    return {"t": "svg", "id": id, "x": av.r(x), "y": av.r(y), "w": av.r(w), "h": av.r(h), "src": ""}


def control(sc, id, x, y, w, h, label, *, bg="panel", color="ink", radius=10,
            event=None, size=15, weight=500, lx=None, lw=None, border=0, bordercolor=None):
    kids = [av.surface(id + "_surface", x, y, w, h, bg=bg, radius=radius,
                       border=border, bordercolor=bordercolor),
            {"t": "button", "id": id + "_control", "x": av.r(x), "y": av.r(y),
             "w": av.r(w), "h": av.r(h), "enabled": 1},
            av.text(id + "_label", label, lx if lx is not None else x,
                    y + (h - 20) / 2, lw if lw is not None else w, 20,
                    weight=weight, color=color, size=size)]
    sc.put(av.stack(id, x, y, w, h, kids,
                    kit=json.dumps({"widget": "KitButton",
                                    "bindings": {"control": [1], "label": [2]}})))
    sc.controls[id] = (event or id, [int(x), int(y), int(w), int(h)], True)


BRACKET = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
           'stroke="#1D1D1F" stroke-width="2" stroke-linecap="round">'
           '<path d="{d}"/></svg>\n')


def build_01(sc):
    # title (gutter caption "1. Pair this device" is NOT UI)
    s, x, y, w, h = sc.rows[find(sc, "Pair with Octos")]
    sc.add_text("t_title", find(sc, "Pair with Octos"), weight=600, size=20)
    # viewfinder: light grey rounded panel + four corner brackets (OCR read two
    # of them as "7" / "L"; they are graphics, not text)
    sc.put(av.surface("vf", 66, 140, 274, 204, bg="panel", radius=12))
    b = 22
    sc.put(svg_node(sc, "vf_br_tl", BRACKET.format(d="M4 14 V4 H14"), 78, 152, b, b))
    sc.put(svg_node(sc, "vf_br_tr", BRACKET.format(d="M10 4 H20 V14"), 306, 152, b, b))
    sc.put(svg_node(sc, "vf_br_bl", BRACKET.format(d="M4 10 V20 H14"), 78, 310, b, b))
    sc.put(svg_node(sc, "vf_br_br", BRACKET.format(d="M14 10 V20 H4"), 306, 310, b, b))
    # the finder is tappable -> the phone scanner runs (shell QR API, wiring card)
    sc.controls["pair_scan"] = ("pair.scan", [66, 140, 274, 204], True)
    # caption under the finder (two measured lines, muted)
    sc.add_text("t_cap1", find(sc, "Scan the pairing QR"), weight=400, size=14, color="muted")
    sc.add_text("t_cap2", find(sc, "on your computer"), weight=400, size=14, color="muted")
    # divider "or" with hairlines
    _, ox, oy, ow, oh = sc.rows[find(sc, "Or")]
    sc.put(av.surface("div_l", 40, oy + oh / 2, 140, 1, bg="hair", radius=0))
    sc.put(av.surface("div_r", 226, oy + oh / 2, 140, 1, bg="hair", radius=0))
    sc.add_text("t_or", find(sc, "Or"), weight=400, size=12, color="muted")
    # input: field label above (measured) + placeholder per the approved prompt
    sc.add_text("t_link_label", find(sc, "Paste pairing link"), weight=400, size=13, color="muted")
    sc.put(av.input_node("pair_link", 28, 490, 350, 44, "octos://pair?code=\u2026"))
    sc.inputs["pair_link"] = ("pair.paste", [28, 490, 350, 44])
    # black pill "Pair"
    control(sc, "pair_submit", 128, 562, 150, 48, "Pair", bg="ink", color="white",
            radius=24, event="pair.submit", lx=185, lw=36)
    # plain link at the bottom (OCR folded it to "-"; copy per the approved prompt)
    control(sc, "pair_fallback", 78, 624, 250, 30, "Enter server and token instead",
            bg="white", radius=0, color="accent", size=14, weight=400,
            event="pair.fallback", lx=78, lw=250)



CHEV = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
        'stroke="#1D1D1F" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'
        '<path d="M15 4 L7 12 L15 20"/></svg>\n')
SPIN = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
        'stroke="#6E6E73" stroke-width="2" stroke-linecap="round">'
        '<path d="M12 3 a9 9 0 1 1 -8.6 6.2"/></svg>\n')
INFO = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
        'stroke="#6E6E73" stroke-width="1.6" stroke-linecap="round">'
        '<circle cx="12" cy="12" r="9"/><path d="M12 8 h0.01 M12 11 v5"/></svg>\n')


def back_button(sc, id, x, y, event):
    sc.put(svg_node(sc, id + "_chev", CHEV, x, y, 14, 22))
    sc.put({"t": "button", "id": id + "_control", "x": av.r(x - 10), "y": av.r(y - 9),
            "w": av.r(34), "h": av.r(40), "enabled": 1})
    sc.controls[id] = (event, [int(x - 10), int(y - 9), 34, 40], True)


def build_02(sc):
    # gutter caption "2. Pairing..." is NOT UI; back chevron is (OCR "<")
    back_button(sc, "pair_back", 17, 58, "pair.back")
    sc.add_text("t_title", find(sc, "Pair with Octos"), weight=600, size=20)
    # centred spinner (prompt: "a centred small spinner"; no OCR ink - authored graphic)
    sc.put(svg_node(sc, "spinner", SPIN, 191, 288, 24, 24))
    sc.add_text("t_pairing", find(sc, "Pairing with"), weight=500, size=16)
    sc.add_text("t_once", find(sc, "This code works once."), weight=400, size=13, color="muted")
    s, x, y, w, h = sc.rows[find(sc, "Cancel")]
    control(sc, "pair_cancel", 203 - 70, 622, 140, 44, "Cancel", bg="white", radius=22,
            border=1, bordercolor="hair", event="pair.cancel", lx=int(x), lw=int(w) + 2)


def build_03(sc):
    sc.add_text("t_title", find(sc, "Pair with Octos"), weight=600, size=20)
    # light-red callout: two measured lines inside one #FDECEC rounded box
    _, x1, y1, w1, h1 = sc.rows[find(sc, "This pairing link")]
    _, x2, y2, w2, h2 = sc.rows[find(sc, "Ask Octos")]
    sc.put(av.surface("cal_red", 28, y1 - 16, 350, (y2 + h2) - (y1 - 16) + 16,
                      bg="cal_red", radius=12))
    sc.put(av.text("t_cal1", sc.t(find(sc, "This pairing link")), x1, y1, w1, h1,
                   size=14, weight=500, color="red_tx"))
    sc.put(av.text("t_cal2", sc.t(find(sc, "Ask Octos")), x2, y2, w2, h2,
                   size=13, weight=400, color="muted"))
    # connect form (prompt: Server prefilled, empty Access token, black Connect pill)
    sc.add_text("t_srv_label", find(sc, "Server"), size=13, color="muted")
    n = av.input_node("connect_server", 28, 322, 350, 48, "")
    n["text"] = "http://192.168.1.20:50190"
    n["color"] = av.C["ink"]
    sc.put(n)
    sc.inputs["connect_server"] = ("connect.server", [28, 322, 350, 48])
    sc.add_text("t_tok_label", find(sc, "Access token"), size=13, color="muted")
    sc.put(av.input_node("connect_token", 28, 436, 350, 48, ""))
    sc.inputs["connect_token"] = ("connect.token", [28, 436, 350, 48])
    control(sc, "connect_submit", 128, 600, 150, 48, "Connect", bg="ink", color="white",
            radius=24, event="connect.submit", lx=160, lw=72)


def build_04(sc):
    # gutter caption "4. Can't pair" and the y0 "enter server and token instead"
    # line are NOT this screen's UI (the latter is screen 1's bottom link caught
    # by the tile crop bleed); this screen per the prompt: two grey callouts + pill
    sc.add_text("t_title", find(sc, "Pair with Octos"), weight=600, size=20)
    _, a, ya, wa, ha = sc.rows[find(sc, "This server doesn't support")]
    _, b, yb, wb, hb = sc.rows[find(sc, "pairing.")]
    sc.put(av.surface("cal_nosup", 28, ya - 16, 350, (yb + hb) - (ya - 16) + 16,
                      bg="panel", radius=12))
    sc.put(svg_node(sc, "cal_info", INFO, 70, ya - 2, 20, 20))
    sc.put(av.text("t_cal1", sc.t(find(sc, "This server doesn't support")), a, ya, wa, ha,
                   size=14, weight=500))
    sc.put(av.text("t_cal2", sc.t(find(sc, "pairing.")), b, yb, wb, hb, size=14, weight=500))
    _, c, yc, wc, hc = sc.rows[find(sc, "Octos on another")]
    _, d, yd, wd, hd = sc.rows[find(sc, "must be paired")]
    _, e, ye, we, he = sc.rows[find(sc, "that computer.")]
    sc.put(av.surface("cal_other", 28, yc - 16, 350, (ye + he) - (yc - 16) + 16,
                      bg="panel", radius=12))
    sc.put(av.text("t_cal3", sc.t(find(sc, "Octos on another")), c, yc, wc, hc, size=14, weight=400))
    sc.put(av.text("t_cal4", sc.t(find(sc, "must be paired")), d, yd, wd, hd, size=14, weight=400))
    sc.put(av.text("t_cal5", sc.t(find(sc, "that computer.")), e, ye, we, he, size=14, weight=400))
    s, x, y, w, h = sc.rows[find(sc, "Use server and token")]
    control(sc, "pair_fallback", 103, 562, 200, 48, "Use server and token", bg="ink",
            color="white", radius=24, event="pair.fallback", lx=int(x), lw=int(w) + 2)


def build_05(sc):
    # gutter caption "5. Paired" is NOT UI
    back_button(sc, "pair_back", 15, 130, "pair.back")
    sc.add_text("t_title", find(sc, "Connection"), weight=600, size=24)
    sc.put(av.surface("conn_card", 28, 228, 350, 196, bg="white", radius=12, border=1,
                      bordercolor="hair"))
    sc.put(av.surface("conn_div1", 36, 296, 334, 1, bg="hair", radius=0))
    sc.put(av.surface("conn_div2", 36, 366, 334, 1, bg="hair", radius=0))
    sc.add_text("t_srv_k", find(sc, "Server"), size=15, weight=500)
    sc.add_text("t_srv_v", find(sc, "192.168.1.20"), size=15)
    sc.add_text("t_pair_k", find(sc, "Paired"), size=15, weight=500)
    sc.add_text("t_pair_v", find(sc, "Today, 9:41 PM"), size=15)
    sc.add_text("t_stays", find(sc, "Stays on this device only"), size=13, color="muted")
    s, x, y, w, h = sc.rows[find(sc, "Forget this device")]
    control(sc, "pair_forget", 117, 608, 172, 40, "Forget this device", bg="white",
            radius=0, color="red_tx", size=16, weight=500, event="pair.forget",
            lx=int(x), lw=int(w) + 2)



EYE = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
       'stroke="#6E6E73" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'
       '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/></svg>\n')
CHECK = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
         'stroke="#1D1D1F" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'
         '<path d="M4 12 l5 5 L20 6"/></svg>\n')
FOLDER = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
          'stroke="#1D1D1F" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">'
          '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>\n')


def provider_editor(sc, rejected):
    # p4-07 has a back chevron; p4-06 does not (per the atlas)
    if rejected:
        back_button(sc, "prov_back", 16, 82, "provider.back")
    sc.add_text("t_title", find(sc, "Edit provider"), weight=600, size=20)
    sc.add_text("t_name_label", find(sc, "Name"), size=13, color="muted")
    n = av.input_node("prov_name", 28, 218, 350, 44, "")
    n["text"] = sc.t(find(sc, "DeepSeek")); n["color"] = av.C["ink"]
    sc.put(n); sc.inputs["prov_name"] = ("provider.name", [28, 218, 350, 44])
    sc.add_text("t_url_label", find(sc, "Base URL"), size=13, color="muted")
    n = av.input_node("prov_url", 28, 296, 350, 44, "")
    n["text"] = sc.t(find(sc, "https://api")); n["color"] = av.C["ink"]
    sc.put(n); sc.inputs["prov_url"] = ("provider.url", [28, 296, 350, 44])
    sc.add_text("t_key_label", find(sc, "API key"), size=13, color="muted")
    key_kwargs = {}
    if rejected:
        # the rejected key field is outlined red (prompt: field outlined red)
        key_kwargs = {"border": 1, "bordercolor": "cal_red"}
        n = av.input_node("prov_key", 28, 372, 350, 44, "", **{})
        n["bg"] = av.C["white"]; n["border_color"] = av.C["red_tx"]; n["border_size"] = 1.0
    else:
        n = av.input_node("prov_key", 28, 374, 350, 44, "")
    n["text"] = "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022"
    n["color"] = av.C["ink"]
    sc.put(n); sc.inputs["prov_key"] = ("provider.key", [28, 372 if rejected else 374, 350, 44])
    sc.put(svg_node(sc, "key_eye", EYE, 350, 386, 20, 20))
    sc.put({"t": "button", "id": "key_eye_control", "x": av.r(342), "y": av.r(378),
            "w": av.r(36), "h": av.r(36), "enabled": 1})
    sc.controls["key_eye"] = ("provider.key.reveal", [342, 378, 36, 36], True)
    if rejected:
        _, a, ya, wa, ha = sc.rows[find(sc, "The provider rejected")]
        _, b, yb, wb, hb = sc.rows[find(sc, "Your draft is kept.")]
        sc.put(av.surface("cal_red", 28, ya - 14, 350, (yb + hb) - (ya - 14) + 14,
                          bg="cal_red", radius=12))
        sc.put(av.text("t_cal1", sc.t(find(sc, "The provider rejected")), a, ya, wa, ha,
                       size=14, weight=500, color="red_tx"))
        sc.put(av.text("t_cal2", sc.t(find(sc, "Your draft is kept.")), b, yb, wb, hb,
                       size=13, weight=400, color="muted"))
    sc.add_text("t_models_label", find(sc, "Models"), weight=600, size=15)
    models = [("deepseek-v4-flash", "provider.model.0"), ("deepseek-v4", "provider.model.1"),
              ("deepseek-chat", "provider.model.2")]
    ys = [462, 503, 545] if not rejected else [394, 430, 466]
    for (prefix, event), ry in zip(models, ys):
        i = find(sc, prefix)
        txt, tx, ty, tw, th = sc.rows[i]
        sc.put(svg_node(sc, f"chk_{event[-1]}", CHECK, 40, ty - 2, 18, 18))
        sc.put(av.text("t_model_" + event[-1], txt, tx, ty, tw, th, size=14, weight=500))
        sc.put({"t": "button", "id": f"model_{event[-1]}_control", "x": av.r(28),
                "y": av.r(ry), "w": av.r(350), "h": av.r(34), "enabled": 1})
        sc.controls["model_row_" + event[-1]] = (event, [28, ry, 350, 34], True)
    if not rejected:
        _, cx, cy, cw, chh = sc.rows[find(sc, "Cancel")]
        control(sc, "prov_cancel", 60, 606, 120, 44, "Cancel", bg="white", radius=22,
                border=1, bordercolor="hair", event="provider.cancel", lx=int(cx), lw=int(cw) + 2)
        sx = sc.rows[find(sc, "Save")][1]
        control(sc, "prov_save", 226, 606, 120, 44, "Save", bg="ink", color="white",
                radius=24, event="provider.save", lx=int(sx), lw=42)
    else:
        _, cx, cy, cw, chh = sc.rows[find(sc, "Cancel")]
        control(sc, "prov_cancel", 40, 518, 120, 40, "Cancel", bg="white", radius=20,
                border=1, bordercolor="hair", event="provider.cancel", lx=int(cx), lw=int(cw) + 2)
        sx = sc.rows[find(sc, "Try again")][1]
        control(sc, "prov_retry", 216, 518, 150, 40, "Try again", bg="ink", color="white",
                radius=20, event="provider.retry", lx=int(sx), lw=64)


def build_06(sc):
    provider_editor(sc, rejected=False)


def build_07(sc):
    provider_editor(sc, rejected=True)


def build_08(sc):
    sc.add_text("t_title", find(sc, "Choose workspace folder"), weight=600, size=20)
    sc.add_text("t_crumbs", find(sc, "/ > Users"), size=13, color="muted")
    rows = [("octos", 196, "browser.enter.0"), ("octoscode-app", 248, "browser.enter.1"),
            ("notes", 308, "browser.enter.2"), ("scratch", 364, "browser.enter.3")]
    for name, ry, event in rows:
        i = find(sc, name)
        txt, tx, ty, tw, th = sc.rows[i]
        selected = name == "octoscode-app"
        sc.put(av.surface(f"row_{event[-1]}", 28, ry, 350, 48,
                          bg="d2" if selected else "white", radius=12))
        sc.put(svg_node(sc, f"fld_{event[-1]}", FOLDER, 46, ry + 15, 18, 18))
        sc.put(av.text("t_row_" + event[-1], txt, tx, ty, tw, th, size=14, weight=500))
        sc.put({"t": "button", "id": f"row_{event[-1]}_control", "x": av.r(28),
                "y": av.r(ry), "w": av.r(350), "h": av.r(48), "enabled": 1})
        sc.controls["browser_row_" + event[-1]] = (event, [28, ry, 350, 48], True)
    sc.add_text("t_hidden", find(sc, "3 hidden by the server"), size=13, color="muted")
    n = av.input_node("browser_path", 28, 490, 350, 48, "")
    n["text"] = sc.t(find(sc, "/Users/dev/code")); n["color"] = av.C["ink"]
    sc.put(n); sc.inputs["browser_path"] = ("browser.path", [28, 490, 350, 48])
    control(sc, "browser_use", 28, 560, 350, 48, "Use this folder", bg="ink", color="white",
            radius=24, event="browser.use", lx=126, lw=132)


def build_09(sc):
    sc.add_text("t_title", find(sc, "Choose workspace folder"), weight=600, size=20)
    sc.add_text("t_crumbs", find(sc, "/ \u203a private"), size=13, color="muted")
    _, a, ya, wa, ha = sc.rows[find(sc, "The server won't list")]
    _, b, yb, wb, hb = sc.rows[find(sc, "Pick another folder")]
    _, c, yc, wc, hc = sc.rows[find(sc, "you can access.")]
    sc.put(av.surface("cal_refused", 28, ya - 16, 350, (yc + hc) - (ya - 16) + 16,
                      bg="panel", radius=12))
    sc.put(av.text("t_cal1", sc.t(find(sc, "The server won't list")), a, ya, wa, ha,
                   size=14, weight=500))
    sc.put(av.text("t_cal2", sc.t(find(sc, "Pick another folder")), b, yb, wb, hb,
                   size=13, weight=400, color="muted"))
    sc.put(av.text("t_cal3", sc.t(find(sc, "you can access.")), c, yc, wc, hc,
                   size=13, weight=400, color="muted"))
    _, bx, by, bw, bh = sc.rows[find(sc, "Back to /Users/dev")]
    control(sc, "browser_back", 28, 382, 350, 44, "Back to /Users/dev", bg="white",
            radius=22, border=1, bordercolor="hair", event="browser.back",
            lx=int(bx), lw=int(bw) + 2)
    n = av.input_node("browser_path", 28, 488, 350, 48, "")
    n["text"] = "/private"; n["color"] = av.C["ink"]
    sc.put(n); sc.inputs["browser_path"] = ("browser.path", [28, 488, 350, 48])


BUILDERS = {1: build_01, 2: build_02, 3: build_03, 4: build_04, 5: build_05, 6: build_06, 7: build_07, 8: build_08, 9: build_09}
SOURCE = {1: "pairing", 2: "pairing", 3: "pairing", 4: "pairing", 5: "pairing",
          6: "provider-editor", 7: "provider-editor", 8: "workspace-browser",
          9: "workspace-browser"}


def build(num):
    sc = av.Scene(num, load_rows(num))
    BUILDERS[num](sc)
    doc = {"schema_version": 1, "id": f"p4-{num:02d}", "app": "octoscode", "number": num,
           "title": TITLES[num],
           "structure": "Native component reconstructed from the approved phase4-new atlas (#D1)",
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                       "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
           "content_source": "Approved stage-a atlas + measured Apple Vision OCR bounds",
           "graphics": {},
           "tree": av.stack("page", 0, 0, 406, 776, sc.kids, variant="surface", bg=av.C["white"])}
    return doc, sc


if __name__ == "__main__":
    import sys
    nums = [int(a) for a in sys.argv[1:]] or sorted(BUILDERS)
    for n in nums:
        doc, sc = build(n)
        d = ROOT / "cards" / f"p4-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        for iid, content in sc.icons.items():
            (d / "assets" / f"{iid}.svg").write_text(content)
        controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                    for cid, (ev, b, en) in sc.controls.items()}
        for cid, (ev, b) in sc.inputs.items():
            controls[cid] = {"event": ev, "source_bounds": b, "enabled": True}
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": controls, "source": SOURCE[n]}, indent=2) + "\n")
        print(f"wrote p4-{n:02d}: {len(sc.kids)} top nodes, {len(sc.icons)} icons, "
              f"{len(controls)} controls")
