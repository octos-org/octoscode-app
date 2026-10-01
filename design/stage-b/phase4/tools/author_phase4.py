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
            event=None, size=15, weight=500, lx=None, lw=None):
    kids = [av.surface(id + "_surface", x, y, w, h, bg=bg, radius=radius),
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


BUILDERS = {1: build_01}
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
