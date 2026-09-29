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
A.ICONS["chev_d"] = '<path d="M6 9l6 6 6-6"/>'

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
    reg("s7_selrow", 7, 330, 250)        # inside the grey chevron chip (233,233,234)
    reg("s7_pill1", 7, 52, 361)
    reg("s7_pill2", 7, 196, 361)
    reg("s7_dot", 7, 284, 606)
    sc.put(A.surface("card_deepseek", 16, 118, 374, 292, bg="s7_card", radius=12, border=1,
                     bordercolor="hair"))
    sc.put(A.surface("card_kimi", 16, 448, 374, 100, bg="s7_card", radius=12))
    sc.put(A.surface("card_glm", 16, 570, 374, 106, bg="s7_card", radius=12))
    sc.add_text("t_title", 0, weight=600, size=19)
    sc.add_text("t_ds_head", 1, weight=600, size=15)
    sc.add_text("t_ds_count", 2, color="muted", size=13)
    sc.put(A.surface("chev_chip", 328, 228, 40, 40, bg="s7_selrow", radius=20))
    sc.put(A.icon("icon_chev", "chev_r", 342, 240, 14, 22, color="muted"))
    A.C["s7_check"] = 0xFF2F6FEB   # ref selected-row check is standard blue
    sc.put(A.icon("icon_check", "check", 299, 240, 16, 16, color="blue"))
    reg("s7_dot_ds", 7, 284, 293)    # DeepSeek head status dot
    sc.put(A.surface("dot_ds", 280, 288, 11, 11, bg="s7_dot_ds", radius=6))
    sc.put(A.icon("icon_chev_ds", "chev_d", 346, 283, 18, 24, color="muted"))
    reg("s7_dot_kimi", 7, 284, 508)  # Kimi head status dot
    sc.put(A.surface("dot_kimi", 280, 503, 11, 11, bg="s7_dot_kimi", radius=6))
    sc.put(A.icon("icon_chev_kimi", "chev_d", 346, 498, 18, 24, color="muted"))
    sc.add_text("t_flash", 3, weight=500, size=14)
    sc.add_text("t_pro", 4, size=14)
    sc.add_control("btn_test", 44, 356, 106, 40, 5, bg="white", radius=20, weight=500,
                   border=1, bordercolor="ink",
                   lx=65, ly=369, lw=76, lh=20, event="models.test_route")
    sc.add_control("btn_discover", 190, 356, 140, 40, 6, bg="white", radius=20, weight=500,
                   border=1, bordercolor="ink",
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
    sc.put(A.surface("modal", 40, 93, 326, 570, bg="s8_modal", radius=12))  # ref white col-run y93..663
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
    reg("s9_track", 9, 300, 150)      # grey track right of the fill (232,231,234)
    reg("s9_blue", 9, 120, 150)       # blue usage fill (38,110,239)
    reg("s9_segborder", 9, 200, 465)  # segmented container border (216,217,217)
    reg("s9_border", 9, 203, 514)     # pill border pixel (71,71,71)
    sc.add_text("t_title", 0, weight=600, size=19)
    sc.add_text("t_usage", 1, size=14)
    sc.add_text("t_pct", 2, color="muted", size=13)
    sc.put(A.surface("bar_track", 45, 142.5, 317, 15.5, bg="s9_track", radius=8))
    sc.put(A.surface("bar_fill", 45, 142.5, 205, 15.5, bg="s9_blue", radius=8))
    for li, vi in [(3, 6), (4, 7), (5, 8)]:
        ls, lx, ly, lw, lh = sc.rows[li]
        sc.put(A.text(f"t_row{li}", ls, lx, ly, lw, lh, size=14))
        vs, vx, vy, vw, vh = sc.rows[vi]
        sc.put(A.text(f"t_val{vi}", vs, vx, vy, vw, vh, color="muted", size=13))
    sc.add_text("t_comp", 9, size=14)
    sc.put(A.surface("seg_box", 160.5, 408, 207.5, 58, bg="white", radius=29,
                     border=1, bordercolor="s9_segborder"))
    sc.add_text("t_llm", 10, weight=500, size=13)
    sc.add_text("t_heur", 11, color="muted", size=13)
    sc.add_control("btn_compact", 60, 513, 290, 44, 12, bg="white", radius=22, weight=600,
                   border=1, bordercolor="s9_border", color="ink",
                   lx=134, ly=534, lw=139, lh=25, event="context.compact_now")
    sc.add_text("t_keep", 13, color="muted", size=13)

def build_10(sc):
    reg("s10_input", 10, 200, 95)
    reg("s10_install", 10, 313, 480)   # install pill fill (253,253,253)
    A.C["s10_hair"] = 0xFFF1F1F1      # full-width hairline grey (241)
    A.C["s10_pillb"] = 0xFF3C3C3E     # install pill outline (60,60,62)
    A.C["s10_inkdark"] = 0xFF212122   # ref placeholder ink (33,33,34)
    A.C["s10_remove"] = 0xFFD3DEFA     # ref (211,222,250) @ (329,236)
    A.C["s10_blue"] = 0xFF0000E0       # ref Remove glyph darkest (0,0,224)
    sc.add_text("t_title", 0, weight=600, size=19)
    A.C["s10_pill"] = 0xFF5B5B5E      # ref dark input pill (91,91,94) @ (203,111)
    A.C_HEX["s10_pill"] = "#5B5B5E"
    A.C["s10_plight"] = 0xFFF6F7F7    # light text/icon on the pill (246,247,247)
    A.C_HEX["s10_plight"] = "#F6F7F7"
    sc.put(A.surface("search_pill", 51, 100.5, 158, 22, bg="s10_pill", radius=11))
    sc.put(A.surface("hair_installed", 0, 204, 406, 1, bg="s10_hair", radius=0))
    sc.put(A.surface("hair_registry", 0, 602, 406, 1, bg="s10_hair", radius=0))
    sc.put(A.icon("icon_search", "search", 60, 102, 16, 16, color="s10_plight"))
    sc.put(A.text("t_search", sc.rows[1][0], 83, 104, 128, 18, color="s10_plight", size=12))
    sc.add_text("t_inst_head", 2, weight=600, size=15)
    for ni, vi, ri, bi in [(3, 6, 14, 0), (4, 7, 15, 1), (5, 8, 16, 2)]:
        ns, nx, ny, nw, nh = sc.rows[ni]
        sc.put(A.text(f"t_name{ni}", ns, nx, ny, nw, nh, weight=500, size=14))
        vs, vx, vy, vw, vh = sc.rows[vi]
        sc.put(A.text(f"t_ver{vi}", vs, vx, vy, vw, vh, color="muted", size=13))
        rs, rx, ry, rw, rh = sc.rows[ri]
        sc.add_control(f"btn_{bi}_remove", 295, ry - 2, 76, 30, ri, bg="s10_remove", radius=15,
                       weight=500, color="s10_blue", lx=rx, ly=ry, lw=rw, lh=rh,
                       event=f"skills.remove_{bi}")
    sc.add_text("t_reg_head", 9, weight=600, size=15)
    for ni, vi, ri, bi in [(10, 11, 17, 3), (12, 13, 18, 4)]:
        ns, nx, ny, nw, nh = sc.rows[ni]
        sc.put(A.text(f"t_name{ni}", ns, nx, ny, nw, nh, weight=500, size=14))
        vs, vx, vy, vw, vh = sc.rows[vi]
        sc.put(A.text(f"t_ver{vi}", vs, vx, vy, vw, vh, color="muted", size=13))
        rs, rx, ry, rw, rh = sc.rows[ri]
        sc.add_control(f"btn_{bi}_install", 268, ry - 15, 90, 44, ri, bg="s10_install",
                       radius=22, border=1, bordercolor="s10_pillb",
                       weight=500, color="ink", lx=rx, ly=ry, lw=rw, lh=rh,
                       event=f"skills.install_{bi}")

def build_11(sc):
    reg("s11_btn", 11, 60, 430)      # reload fill (16,17,16)
    reg("s11_diagb", 11, 180, 474)   # diag border (206,205,206)
    sc.put(A.icon("icon_warning", "warning", 130, 80, 72, 77, color="ink"))
    sc.add_text("t_title", 0, weight=600, size=22)
    for i in (1, 2, 3):
        sc.add_text(f"t_msg{i}", i, color="muted", size=14)
    sc.add_control("btn_reload", 26, 387, 303, 65, 4, bg="s11_btn", radius=32, weight=600,
                   color="white", lx=141, ly=410, lw=68, lh=25, event="error.reload")
    sc.add_control("btn_diag", 26, 473, 303, 63, 5, bg="white", radius=31, weight=500,
                   border=1, bordercolor="s11_diagb", lx=87, ly=491, lw=164, lh=33,
                   event="error.copy_diagnostics")
    sc.add_text("t_report", 6, weight=500, size=14, color="muted")  # ref darkest (67,67,70)

def build_12(sc):
    reg("s12_banner", 12, 203, 10)
    reg("s12_skel", 12, 200, 124)
    sc.put(A.surface("banner", 0, 0, 406, 58, bg="s12_banner", radius=0))
    s, x, y, w, h = sc.rows[0]
    sc.put(A.text("t_banner", "Reconnecting… attempt 2 ·", 46.2, 29.3, 222, 25.5, weight=500, size=14))
    A.C["s12_blue"] = 0xFF4874DF     # ref (72,116,223) blue link
    sc.put(A.text("t_retry", "Retry now", 276.5, 29, 77, 25, weight=600, size=14, color="s12_blue"))
    for i, (sx, sy, sw, sh) in enumerate([(30, 98, 70, 50), (100, 104, 255, 44),
                                          (30, 180, 70, 56), (100, 184, 255, 52),
                                          (30, 270, 70, 49), (100, 275, 176, 44)]):
        sc.put(A.surface(f"skel{i}", sx, sy, sw, sh, bg="s12_skel", radius=8))
    A.C["s12_div"] = 0xFFEEF0F2   # ref hairline (238,240,242) @ y392
    sc.put(A.surface("divider", 24, 392, 358, 2, bg="s12_div", radius=0))
    sc.put(A.icon("icon_spin", "spinner", 178, 441, 30, 36, color="ink"))
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
