#!/usr/bin/env python3
"""Author the six board-2 setup contracts (entry #28d), board-1 pattern.

Reuses author_v2's node helpers and Scene verbatim (same lowering contract).
Text copy + positions are MEASURED (Apple Vision OCR per setup-XX reference.png,
logical 406x776; rows loaded from design/stage-b/setup/ocr/). Non-text geometry
(bars, skeletons, pills) comes from the viewed panels; FILL COLOURS ARE SAMPLED
from reference.png pixels (cited in the report), not guessed.
"""
import json, sys
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent                 # design/stage-b/setup/tools
SETUP = HERE.parent                                    # design/stage-b/setup
CONV = SETUP.parent / "conversation" / "tools"
sys.path.insert(0, str(CONV))
import author_v2 as A

A.ICONS["warning"] = ('<path d="M12 4L22 20H2z"/>'
                      '<path d="M12 10v5"/><circle cx="12" cy="17.6" r="0.9" fill="#000"/>')
A.ICONS["close"] = '<path d="M6 6l12 12"/><path d="M18 6L6 18"/>'
A.ICONS["chev_r"] = '<path d="M9 6l6 6-6 6"/>'

TITLES = {7: "Model settings", 8: "Command palette", 9: "Context panel",
          10: "Skills", 11: "Error screen", 12: "Loading and reconnecting"}

FIX = {"TV to move • & torun • esc": "↑↓ to move · ↵ to run · esc",
       "Reconnecting... attempt 2 • Retry now": "Reconnecting… attempt 2 · Retry now"}

def load_rows(k):
    d = json.loads((SETUP / "ocr" / f"setup-{k:02d}.ocr.json").read_text())
    sx, sy = 406 / d["width"], 776 / d["height"]
    out = []
    for o in d["observations"]:
        x, y, w, h = o["bounds"]
        out.append((FIX.get(o["text"], o["text"]), x * sx, y * sy, w * sx, h * sy))
    return out

REF = {}
def reg(name, k, x, y):
    """Sample a fill from the reference and register it as a named palette key
    (author_v2.surface() looks colours up in C by name)."""
    A.C[name] = samp(k, x, y)
    return name

def samp(k, x, y):
    """Sample the 2x reference at logical (x, y); cache per screen."""
    if k not in REF:
        REF[k] = Image.open(SETUP / "cards" / f"setup-{k:02d}" / "reference.png").convert("RGB")
    r, g, b = REF[k].getpixel((int(x * 2), int(y * 2)))
    return 0xFF000000 | (r << 16) | (g << 8) | b

def build_07(sc):
    reg("s7_page", 7, 8, 700)
    reg("s7_card", 7, 300, 160)          # right of the heading text run
    reg("s7_selrow", 7, 330, 247)        # inside the selected row, clear of text
    reg("s7_pill1", 7, 60, 376)
    reg("s7_pill2", 7, 300, 376)
    reg("s7_dot", 7, 284, 606)
    sc.put(A.surface("card_deepseek", 16, 118, 374, 292, bg="s7_card", radius=12, border=1,
                     bordercolor="hair"))
    sc.put(A.surface("card_kimi", 16, 448, 374, 100, bg="s7_card", radius=12))
    sc.put(A.surface("card_glm", 16, 570, 374, 106, bg="s7_card", radius=12))
    sc.add_text("t_title", 0, weight=600, size=19)
    sc.add_text("t_ds_head", 1, weight=600, size=15)
    sc.add_text("t_ds_count", 2, color="muted", size=13)
    sc.put(A.surface("row_flash", 24, 224, 358, 46, bg="s7_selrow", radius=8))
    sc.add_text("t_flash", 3, weight=500, size=14)
    sc.add_text("t_pro", 4, size=14)
    sc.add_control("btn_test", 44, 356, 106, 40, 5, bg="s7_pill1", radius=20, weight=500,
                   lx=65, ly=369, lw=76, lh=20, event="models.test_route")
    sc.add_control("btn_discover", 190, 356, 140, 40, 6, bg="s7_pill2", radius=20, weight=500,
                   lx=208, ly=369, lw=112, lh=20, event="models.discover")
    sc.add_text("t_kimi_head", 7, weight=600, size=15)
    sc.add_text("t_kimi_count", 8, color="muted", size=13)
    sc.add_text("t_glm_head", 9, weight=600, size=15)
    sc.add_text("t_glm_count", 10, color="muted", size=13)
    sc.put(A.surface("dot_glm", 280, 601, 11, 11, bg="s7_dot", radius=6))

def build_08(sc):
    reg("s8_scrim", 8, 10, 10)
    reg("s8_modal", 8, 203, 320)
    reg("s8_selrow", 8, 200, 219)
    sc.put(A.surface("scrim", 0, 0, 406, 776, bg="s8_scrim", radius=0))
    sc.put(A.surface("modal", 40, 112, 326, 546, bg="s8_modal", radius=12))
    _s, _x, _y, _w, _h = sc.rows[1]
    sc.put(A.icon("icon_close", "close", _x, _y, _w, _h, color="ink"))
    s, x, y, w, h = sc.rows[0]
    sc.put(A.code("t_query", s, x, y, w, h, weight=500, size=15))
    pairs = [(2, 4), (3, 5), (6, 10), (7, 11), (8, 12), (9, 13)]
    for i, (ci, di) in enumerate(pairs):
        cs, cx, cy, cw, ch = sc.rows[ci]
        if i == 0:
            sc.put(A.surface("row_sel", 48, cy - 14, 310, ch + 28, bg="s8_selrow", radius=8))
        sc.put(A.code(f"t_cmd{i}", cs, cx, cy, cw, ch, weight=500, size=14))
        ds, dx, dy, dw, dh = sc.rows[di]
        sc.put(A.text(f"t_desc{i}", ds, dx, dy, dw, dh, color="muted", size=13))
    hs, hx, hy, hw, hh = sc.rows[14]
    sc.put(A.text("t_hints", hs, hx, hy, hw, hh, color="muted", size=12))

def build_09(sc):
    reg("s9_track", 9, 350, 132)
    reg("s9_fill", 9, 100, 132)
    reg("s9_seg", 9, 185, 425)
    reg("s9_btn", 9, 120, 545)
    sc.add_text("t_title", 0, weight=600, size=19)
    sc.add_text("t_usage", 1, size=14)
    sc.add_text("t_pct", 2, color="muted", size=13)
    sc.put(A.surface("bar_track", 42, 128, 322, 8, bg="s9_track", radius=4))
    sc.put(A.surface("bar_fill", 42, 128, 200, 8, bg="s9_fill", radius=4))
    for li, vi in [(3, 6), (4, 7), (5, 8)]:
        ls, lx, ly, lw, lh = sc.rows[li]
        sc.put(A.text(f"t_row{li}", ls, lx, ly, lw, lh, size=14))
        vs, vx, vy, vw, vh = sc.rows[vi]
        sc.put(A.text(f"t_val{vi}", vs, vx, vy, vw, vh, color="muted", size=13))
    sc.add_text("t_comp", 9, size=14)
    sc.put(A.surface("seg_pill", 182, 419, 84, 32, bg="s9_seg", radius=16))
    sc.add_text("t_llm", 10, weight=500, size=13)
    sc.add_text("t_heur", 11, color="muted", size=13)
    sc.add_control("btn_compact", 103, 522, 200, 46, 12, bg="s9_btn", radius=23, weight=600,
                   color="white", lx=134, ly=534, lw=139, lh=25, event="context.compact_now")
    sc.add_text("t_keep", 13, color="muted", size=13)

def build_10(sc):
    reg("s10_input", 10, 200, 95)
    reg("s10_install", 10, 360, 499)
    sc.add_text("t_title", 0, weight=600, size=19)
    sc.put(A.surface("search_box", 24, 86, 358, 50, bg="s10_input", radius=10))
    sc.put(A.icon("icon_search", "search", 42, 100, 20, 20, color="muted"))
    sc.add_text("t_search", 1, color="muted", size=14)
    sc.add_text("t_inst_head", 2, weight=600, size=15)
    for ni, vi, ri, bi in [(3, 6, 14, 0), (4, 7, 15, 1), (5, 8, 16, 2)]:
        ns, nx, ny, nw, nh = sc.rows[ni]
        sc.put(A.text(f"t_name{ni}", ns, nx, ny, nw, nh, weight=500, size=14))
        vs, vx, vy, vw, vh = sc.rows[vi]
        sc.put(A.text(f"t_ver{vi}", vs, vx, vy, vw, vh, color="muted", size=13))
        rs, rx, ry, rw, rh = sc.rows[ri]
        sc.add_control(f"btn_{bi}_remove", 287, ry - 8, 84, 34, ri, bg="white", radius=17,
                       weight=500, lx=rx, ly=ry, lw=rw, lh=rh, event=f"skills.remove_{bi}")
    sc.add_text("t_reg_head", 9, weight=600, size=15)
    for ni, vi, ri, bi in [(10, 11, 17, 3), (12, 13, 18, 4)]:
        ns, nx, ny, nw, nh = sc.rows[ni]
        sc.put(A.text(f"t_name{ni}", ns, nx, ny, nw, nh, weight=500, size=14))
        vs, vx, vy, vw, vh = sc.rows[vi]
        sc.put(A.text(f"t_ver{vi}", vs, vx, vy, vw, vh, color="muted", size=13))
        rs, rx, ry, rw, rh = sc.rows[ri]
        sc.add_control(f"btn_{bi}_install", 280, ry - 8, 92, 34, ri, bg="s10_install", radius=17,
                       weight=500, color="white", lx=rx, ly=ry, lw=rw, lh=rh,
                       event=f"skills.install_{bi}")

def build_11(sc):
    reg("s11_btn", 11, 150, 401)
    sc.put(A.icon("icon_warning", "warning", 188, 110, 30, 26, color="ink"))
    sc.add_text("t_title", 0, weight=600, size=22)
    for i in (1, 2, 3):
        sc.add_text(f"t_msg{i}", i, color="muted", size=14)
    sc.add_control("btn_reload", 131, 396, 88, 46, 4, bg="s11_btn", radius=23, weight=600,
                   color="white", lx=140, ly=409, lw=69, lh=25, event="error.reload")
    sc.add_control("btn_diag", 69, 480, 200, 50, 5, bg="white", radius=25, weight=500,
                   border=1, bordercolor="hair", lx=86, ly=490, lw=165, lh=34,
                   event="error.copy_diagnostics")
    sc.add_text("t_report", 6, weight=500, size=14, color="blue")

def build_12(sc):
    reg("s12_banner", 12, 203, 10)
    reg("s12_skel", 12, 200, 124)
    sc.put(A.surface("banner", 0, 0, 406, 58, bg="s12_banner", radius=0))
    s, x, y, w, h = sc.rows[0]
    sc.put(A.text("t_banner", s, x, y, w, h, weight=500, size=14))
    for i, (by, bh) in enumerate([(100, 47), (181, 46), (270, 49)]):
        sc.put(A.surface(f"skel{i}", 46, by, 314, bh, bg="s12_skel", radius=8))
    sc.put(A.surface("divider", 24, 470, 358, 1, bg="hair", radius=0))
    sc.put(A.icon("icon_spin", "spinner", 88, 508, 20, 20, color="muted"))
    sc.add_text("t_loading", 1, weight=500, size=15)
    sc.add_text("t_cancel", 2, weight=500, size=14, color="muted")

BUILDERS = {7: build_07, 8: build_08, 9: build_09, 10: build_10, 11: build_11, 12: build_12}

if __name__ == "__main__":
    for k in range(7, 13):
        A.ICON_REG.clear()
        sc = A.Scene(k, load_rows(k))
        BUILDERS[k](sc)
        doc = {"schema_version": 1, "id": f"setup-{k:02d}", "app": "octoscode", "number": k,
               "title": TITLES[k],
               "structure": "Native component reconstructed from the approved stage-a board-2 atlas",
               "artboard": [406, 776], "font_family": "Inter",
               "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                           "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
               "content_source": "Approved stage-a board-2 atlas + measured Apple Vision OCR bounds",
               "graphics": {},
               "tree": A.stack("page", 0, 0, 406, 776, sc.kids, variant="surface", bg=A.C["white"])}
        d = SETUP / "cards" / f"setup-{k:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        for iid, (name, color) in A.ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(A.svg(name, A.C_HEX.get(color, "#1D1D1F")))
        controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                    for cid, (ev, b, en) in sc.controls.items()}
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": k, "controls": controls,
             "source": "authored from the approved stage-a board-2 atlas (Gate A 2026-09-29) + Apple Vision OCR"},
            indent=2) + "\n")
        print(f"wrote setup-{k:02d}: {len(sc.kids)} top nodes, {len(A.ICON_REG)} icons, "
              f"{len(controls)} controls")
