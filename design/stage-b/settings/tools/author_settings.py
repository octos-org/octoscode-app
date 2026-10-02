#!/usr/bin/env python3
"""Author the seven #D2b settings contracts (board 2, screens 6-12).

Board-1/2 pattern (author_setup_7_12.py): author_v2's node helpers + Scene,
text copy + positions MEASURED (Apple Vision OCR per settings-XX
reference.ocr.json; rows loaded per card, scaled to the 406x776 logical
artboard). Non-text geometry (toggles, radios, the segmented control, the
banner, the modal card) comes from the viewed panels; FILL COLOURS ARE
SAMPLED from reference.png pixels (cited in the report), not guessed.

NOTE on aspect: the operator-approved board-2 atlas drew these panels
LANDSCAPE; like every flow stage we author in the 406x776 logical space the
OCR bounds are already scaled into (the squash both gate sides apply,
gate.py:185). The aspect deviation vs 406/776 is recorded in report-D2b.md.
"""
import json, sys
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent                 # design/stage-b/settings/tools
SET = HERE.parent                                      # design/stage-b/settings
CONV = SET.parent / "conversation" / "tools"
sys.path.insert(0, str(CONV))
import author_v2 as A

A.ICONS["chev_l"] = '<path d="M15 6l-6 6 6 6"/>'
A.ICONS["gear"] = ('<circle cx="12" cy="12" r="3.4"/>'
                   '<path d="M12 2v3.2M12 18.8V22M2 12h3.2M18.8 12H22'
                   'M4.8 4.8l2.2 2.2M17 17l2.2 2.2M19.2 4.8L17 7M7 17l-2.2 2.2"/>')
A.ICONS["shield"] = '<path d="M12 3l7 3v5.5c0 4.4-3 7.4-7 8.9-4-1.5-7-4.5-7-8.9V6z"/>'
A.ICONS["code"] = '<path d="M9 8l-4 4 4 4M15 8l4 4-4 4"/>'
A.ICONS["box"] = ('<path d="M21 8l-9-5-9 5v8l9 5 9-5V8z"/>'
                  '<path d="M3.3 7l8.7 5 8.7-5M12 22V12"/>')
A.ICONS["plug"] = '<path d="M9 2v6M15 2v6M7 8h10v3a5 5 0 0 1-10 0V8zM12 16v6"/>'
A.ICONS["info"] = ('<circle cx="12" cy="12" r="9" fill="none" stroke="#1D1D1F" stroke-width="1.6"/>'
                   '<path d="M12 10.5v6"/><circle cx="12" cy="7.6" r="1" fill="#000"/>')

TITLES = {6: "Settings general", 7: "Stop server confirm", 8: "Settings permissions",
          9: "Settings sandbox", 10: "New-chat defaults", 11: "Settings model thinking",
          12: "Held by another client"}

# OCR misreads -> the approved prompt's exact strings
FIX = {"System v": "System ▾", "v4-flash v": "v4-flash ▾", "V": "", "v": "",
       "Stop server...": "Stop server…", "Advanced...": "Advanced…",
       "$": "", "‹>": "", "<>": ""}

def load_rows(k):
    d = json.loads((SET / "cards" / f"settings-{k:02d}" / "reference.ocr.json").read_text())
    sx, sy = 406 / d["width"], 776 / d["height"]
    out = []
    for o in d["observations"]:
        x, y, w, h = o["bounds"]
        t = FIX.get(o["text"], o["text"])
        # noise rows ("‹>" "$" "<>" "V") are kept as EMPTY PLACEHOLDERS so the
        # builders' row indices stay aligned with the dumped OCR lists
        out.append((t, x * sx, y * sy, w * sx, h * sy))
    # EMPTY sentinel row: invisible labels for the hit-area buttons (the last
    # index of every card's rows; renders nothing wherever the lowering puts it)
    out.append(("", 0.0, 0.0, 1.0, 1.0))
    return out

REF = {}
def samp(k, x, y):
    if k not in REF:
        REF[k] = Image.open(SET / "cards" / f"settings-{k:02d}" / "reference.png").convert("RGB")
    im = REF[k]
    sx, sy = im.width / 406.0, im.height / 776.0
    r, g, b = im.getpixel((min(int(x * sx), im.width - 1), min(int(y * sy), im.height - 1)))
    return 0xFF000000 | (r << 16) | (g << 8) | b

def reg(name, k, x, y):
    A.C[name] = samp(k, x, y)
    return name

def toggle(sc, cid, x, y, on):
    sc.put(A.surface(f"{cid}_pill", x, y, 50, 30, bg="blue" if on else "hair", radius=999,
                     kids=[A.surface(f"{cid}_knob", x + (26 if on else 3), y + 3, 24, 24,
                                     bg="white", radius=999)]))
    # the REAL control: a transparent DesignNativeButton ON TOP (the taps
    # machinery wires DesignNativeButton instances only — taps.rs inject_click;
    # the probe proved bare surfaces never wire)
    sc.add_control(cid, x, y, 50, 30, len(sc.rows) - 1, bg="hit", radius=999,
                   event=f"{cid}.toggle")

def radio(sc, cid, x, y, on):
    if on:
        sc.put(A.surface(f"{cid}_ring", x, y, 22, 22, bg="blue", radius=999))
        sc.put(A.surface(f"{cid}_dot", x + 7, y + 7, 8, 8, bg="white", radius=999))
    else:
        sc.put(A.surface(f"{cid}_ring", x, y, 22, 22, bg="white", radius=999, border=1.6,
                         bordercolor="muted"))
    sc.add_control(cid, x, y, 22, 22, len(sc.rows) - 1, bg="hit", radius=999,
                   event=f"{cid}.select")

def rail(sc, src_k):
    """The settings nav rail below the back chevron: the gear row in a tinted
    chip (blue-tinted on the General pages 06/07 = selected, grey on 08/09/11)
    + shield/code/box/plug/info. Geometry measured on the 06 reference (dark
    cluster bboxes in report-D2b.md); chip fills sampled per card."""
    if src_k in (6, 7):
        A.C["rail_chip"] = 0xFFEFF5FE
        A.C["rail_gear"] = 0xFF476EE8
        A.C_HEX["rail_gear"] = "#476EE8"
        gear = "rail_gear"
    else:
        A.C["rail_chip"] = samp(src_k, 32, 108)
        gear = "ink"
    sc.put(A.surface("rail_chip", 11, 121, 42, 50, bg="rail_chip", radius=10))
    sc.put(A.icon("rail_gear_i", "gear", 16, 130, 24, 34, color=gear))
    sc.put(A.icon("rail_shield_i", "shield", 16, 218, 24, 48))
    sc.put(A.icon("rail_code_i", "code", 16, 318, 24, 34))
    sc.put(A.icon("rail_box_i", "box", 16, 404, 24, 54))
    sc.put(A.icon("rail_plug_i", "plug", 16, 500, 24, 50))
    sc.put(A.icon("rail_info_i", "info", 16, 592, 24, 48))

def hairline(sc, cid, x, y, w):
    sc.put(A.surface(cid, x, y, w, 1, bg="hair", radius=0))

# ---------------------------------------------------------------- 06 General
def build_06(sc):
    rail(sc, 6)
    reg("s6_red", 6, 90, 651)          # the Stop server… row's red text
    sc.put(A.icon("back", "chev_l", 18, 44, 22, 22, color="ink"))
    sc.add_text("t_title", 0, weight=600, size=19)
    hairline(sc, "div1", 24, 118, 358)
    sc.add_text("t_notif", 1, weight=500, size=15)
    toggle(sc, "notifications_toggle", 330, 148, True)
    sc.add_text("t_help1", 2, color="muted", size=12.5)
    sc.add_text("t_help2", 3, color="muted", size=12.5)
    sc.add_text("t_help3", 4, color="muted", size=12.5)
    hairline(sc, "div2", 24, 372, 358)
    sc.add_text("t_theme", 6, weight=500, size=15)
    sc.add_text("t_theme_v", 7, color="muted", size=14.5)
    hairline(sc, "div3", 24, 487, 358)
    sc.add_text("t_server", 8, weight=500, size=15)
    sc.add_text("t_server_v", 9, color="muted", size=13.5)
    hairline(sc, "div4", 24, 604, 358)
    sc.put(A.text("t_stop", sc.rows[10][0], 73.2, 635.3, sc.rows[10][3], sc.rows[10][4],
                  weight=500, size=15, color="s6_red"))
    sc.add_control("stop_row", 24, 620, 358, 62, len(sc.rows) - 1, bg="hit", radius=12,
                   event="server.stop.request")
    sc.add_text("t_stop_help", 11, color="muted", size=12.5)

# ------------------------------------------------------ 07 Stop-server confirm
def build_07(sc):
    reg("s7_scrim", 7, 15, 700)        # dimmed settings behind the modal
    reg("s7_red", 7, 276, 512)         # the red-filled button
    # the dimmed General page behind: rail first, then the scrim over it
    rail(sc, 6)
    sc.put(A.surface("dim", 0, 0, 406, 776, bg="s7_scrim", radius=0))
    sc.add_text("b_title", 1, weight=600, size=19)
    sc.add_text("b_sec", 2, weight=600, size=15)
    sc.put(A.text("b_stop", sc.rows[8][0], 73.2, 635.3, sc.rows[8][3], sc.rows[8][4],
                  weight=500, size=15, color="muted"))
    sc.add_text("b_stop_help", 9, color="muted", size=12.5)
    # centred modal card
    sc.put(A.surface("modal", 28, 176, 350, 372, bg="white", radius=12, border=1,
                     bordercolor="hair"))
    sc.add_text("m_title", 3, weight=600, size=16.5)
    sc.add_text("m_body1", 4, color="muted", size=13.5)
    sc.add_text("m_body2", 5, color="muted", size=13.5)
    sc.put(A.surface("cancel_btn_fill", 58, 480, 128, 46, bg="white", radius=999, border=1,
                     bordercolor="hair"))
    # NOTE: NOT "cancel_btn_label" — add_control("cancel_btn") auto-generates
    # id+"_label" for its (empty-sentinel) label, and a duplicate id fails
    # semantic preflight ("duplicate semantic element IDs")
    sc.put(A.text("cancel_label", sc.rows[6][0], 121.0, 490.4, 47.0, 33.7,
                  weight=500, size=14.5))
    # the hit key uses the EMPTY sentinel row (a non-empty label makes the
    # KitButton layout shift the lowered abs_pos — measured (89,467) vs the
    # authored (58,480), x+31/y-13, far outside taps' 1.5px tolerance; the
    # sentinel-labelled confirm lowers exactly, probe-proven)
    sc.add_control("cancel_btn", 58, 480, 128, 46, len(sc.rows) - 1, bg="hit", radius=999,
                   event="server.stop.cancel")
    sc.put(A.surface("stop_btn_fill", 220, 480, 128, 46, bg="s7_red", radius=999))
    sc.put(A.text("stop_btn_label", sc.rows[7][0], 237.3, 489.9, 78.0, 45.9,
                  weight=600, size=14.5, color="white"))
    sc.add_control("stop_confirm", 220, 480, 128, 46, len(sc.rows) - 1, bg="hit", radius=999,
                   event="server.stop.confirm")

# ------------------------------------------------------------- 08 Permissions
def build_08(sc):
    rail(sc, 8)
    sc.put(A.icon("back", "chev_l", 18, 44, 22, 22, color="ink"))
    sc.add_text("t_title", 1, weight=600, size=19)
    radio(sc, "perm_ask", 46, 228, True)
    sc.put(A.text("t_opt1", sc.rows[3][0][2:], 72.8, 222.1, sc.rows[3][3], sc.rows[3][4],
                  weight=500, size=15))
    sc.add_text("t_opt1_d", 4, color="muted", size=12.5)
    radio(sc, "perm_workspace", 46, 348, False)
    sc.put(A.text("t_opt2", sc.rows[5][0][2:], 72.8, 342.8, sc.rows[5][3], sc.rows[5][4],
                  weight=500, size=15))
    sc.add_text("t_opt2_d", 6, color="muted", size=12.5)
    radio(sc, "perm_full", 46, 470, False)
    sc.add_text("t_opt3", 7, weight=500, size=15)
    sc.add_text("t_opt3_d", 8, color="muted", size=12.5)
    hairline(sc, "div", 24, 596, 358)
    sc.add_text("t_readback", 9, color="muted", size=13)
    A.C["s8_blue"] = 0xFF2F6FEB
    sc.put(A.text("t_advanced", sc.rows[10][0], 72.7, 687.6, sc.rows[10][3], sc.rows[10][4],
                  weight=500, size=13.5, color="s8_blue"))
    sc.add_control("advanced_link", 64, 680, 120, 40, len(sc.rows) - 1, bg="hit", radius=8,
                   event="settings.advanced")

# ---------------------------------------------------------------- 09 Sandbox
def build_09(sc):
    rail(sc, 9)
    sc.put(A.icon("back", "chev_l", 18, 44, 22, 22, color="ink"))
    sc.add_text("t_title", 2, weight=600, size=19)
    sc.add_text("t_row1", 3, weight=500, size=15)
    toggle(sc, "sandbox_write", 330, 222, True)
    sc.add_text("t_row1_d", 4, color="muted", size=12.5)
    sc.add_text("t_row2", 5, weight=500, size=15)
    toggle(sc, "sandbox_network", 330, 369, False)
    sc.add_text("t_row2_d", 6, color="muted", size=12.5)
    sc.add_text("t_row3", 7, weight=500, size=15)
    toggle(sc, "sandbox_read_outside", 330, 517, False)
    sc.add_text("t_row3_d", 8, color="muted", size=12.5)
    hairline(sc, "div", 24, 674, 358)
    sc.add_text("t_foot", 9, color="muted", size=12.5)

# ------------------------------------------------------- 10 New-chat defaults
def build_10(sc):
    reg("s10_strip", 10, 200, 60)      # the strip's light fill
    sc.put(A.surface("strip", 0, 0, 406, 128, bg="s10_strip", radius=0))
    sc.add_text("t_strip1", 0, weight=500, size=13.5)
    sc.add_text("t_strip2", 1, weight=500, size=13.5)
    A.C["s10_blue"] = 0xFF2F6FEB
    sc.put(A.text("t_change", sc.rows[2][0], 335.8, 92.6, sc.rows[2][3], sc.rows[2][4],
                  weight=500, size=13.5, color="s10_blue"))
    sc.add_control("defaults_change", 326, 84, 70, 42, len(sc.rows) - 1, bg="hit", radius=8,
                   event="settings.defaults.open")
    sc.add_text("t_placeholder", 3, color="muted", size=16, weight=500)
    sc.add_text("t_placeholder_sub", 4, color="muted", size=12.5)
    sc.put(A.input_node("composer", 20, 580, 366, 84, sc.rows[5][0]))

# --------------------------------------------------- 11 Model + Thinking seg
def build_11(sc):
    rail(sc, 11)
    sc.put(A.icon("back", "chev_l", 18, 44, 22, 22, color="ink"))
    sc.add_text("t_title", 0, weight=600, size=19)
    sc.add_text("t_sec", 1, weight=600, size=15)
    sc.add_text("t_model", 2, weight=500, size=15)
    sc.add_text("t_model_v", 3, color="muted", size=14.5)
    hairline(sc, "div", 24, 316, 358)
    sc.add_text("t_think", 4, weight=500, size=15)
    # segmented control: container + selected pill on "On"
    reg("s11_seg", 11, 200, 345)       # the segmented track fill
    reg("s11_sel", 11, 268, 372)       # the selected segment's fill
    sc.put(A.surface("seg_track", 170, 348, 196, 44, bg="s11_seg", radius=999))
    sc.put(A.surface("seg_sel", 236, 351, 62, 38, bg="s11_sel", radius=999))
    sc.put(A.text("seg_off", sc.rows[5][0], 181.8, 357.1, 23.9, 29.5, size=13.5, color="muted"))
    sc.put(A.text("seg_on", sc.rows[6][0], 258.7, 357.1, 21.2, 32.2, size=13.5, weight=600))
    sc.put(A.text("seg_high", sc.rows[8][0], 330.4, 359.8, 29.2, 32.2, size=13.5, color="muted"))
    for cid, ev, bx in (("think_off", "settings.thinking.off", 170),
                        ("think_on", "settings.thinking.on", 236),
                        ("think_high", "settings.thinking.high", 298)):
        sc.add_control(cid, bx, 348, 62, 44, len(sc.rows) - 1, bg="hit", radius=999, event=ev)
    sc.add_text("t_help", 7, color="muted", size=12.5)

# ------------------------------------------------- 12 Held by another client
def build_12(sc):
    reg("s12_banner", 12, 200, 40)     # the banner's fill
    reg("s12_ghost", 12, 30, 690)      # the greyed composer fill
    sc.put(A.surface("banner", 0, 0, 406, 132, bg="s12_banner", radius=0))
    sc.put(A.icon("info_i", "info", 24, 40, 20, 20, color="ink"))
    sc.add_text("t_ban1", 0, weight=500, size=13.5)
    sc.add_text("t_ban2", 1, color="muted", size=12.5)
    sc.put(A.surface("takeover_fill", 312, 56, 82, 44, bg="black", radius=999))
    sc.put(A.text("takeover_label", sc.rows[2][0], 325.6, 63.9, 50.1, 32.7,
                  weight=600, size=13, color="white"))
    sc.add_control("take_over", 312, 56, 82, 44, len(sc.rows) - 1, bg="hit", radius=999,
                   event="session.take_over")
    hairline(sc, "ban_div", 0, 132, 406)
    # the read-along thread (measured rows)
    sc.add_text("t_thread1", 3, weight=600, size=14.5)
    sc.add_text("t_thread2", 4, color="muted", size=12.5)
    sc.add_text("t_b1", 5, size=13.5)
    sc.add_text("t_b2", 6, size=13.5)
    sc.add_text("t_b3", 8, size=13.5)
    sc.put(A.chip("chip_patch", sc.rows[9][0], 24.5, 513, 44, 30, size=12))
    sc.put(A.text("t_add12", sc.rows[10][0], 132.4, 519.1, 17.7, 24.1, size=12.5, color="s12_add"))
    sc.put(A.text("t_del4", sc.rows[11][0], 167.7, 519.1, 17.7, 21.4, size=12.5, color="s12_del"))
    sc.add_text("t_done", 12, color="muted", size=13)
    sc.put(A.surface("ghost_composer", 20, 660, 366, 84, bg="s12_ghost", radius=24,
                     border=1, bordercolor="hair"))
    sc.add_text("t_readonly", 13, color="muted", size=12.5)

A.C["s12_add"] = 0xFF1F883D
A.C["s12_del"] = 0xFFCF222E
A.C["hit"] = 0x00000000   # transparent hit-area buttons (taps.rs wires DesignNativeButton only)
BUILDERS = {6: build_06, 7: build_07, 8: build_08, 9: build_09, 10: build_10,
            11: build_11, 12: build_12}

if __name__ == "__main__":
    for k in range(6, 13):
        A.ICON_REG.clear()
        sc = A.Scene(k, load_rows(k))
        BUILDERS[k](sc)
        doc = {"schema_version": 1, "id": f"settings-{k:02d}", "app": "octoscode", "number": k,
               "title": TITLES[k],
               "structure": "Native component reconstructed from the approved stage-a board-2 atlas",
               "artboard": [406, 776], "font_family": "Inter",
               "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                           "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
               "content_source": "Approved stage-a board-2 atlas + measured Apple Vision OCR bounds",
               "graphics": {},
               "tree": A.stack("page", 0, 0, 406, 776, sc.kids, variant="surface", bg=A.C["white"])}
        d = SET / "cards" / f"settings-{k:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        for iid, (name, color) in A.ICON_REG.items():
            (d / "assets" / f"{iid}.svg").write_text(A.svg(name, A.C_HEX.get(color, "#1D1D1F")))
        controls = {cid: {"event": ev, "source_bounds": b, "enabled": en}
                    for cid, (ev, b, en) in sc.controls.items()}
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": k, "controls": controls,
             "source": "authored from the approved stage-a board-2 atlas (operator 2026-10-01) + Apple Vision OCR"},
            indent=2) + "\n")
        print(f"wrote settings-{k:02d}: {len(sc.kids)} top nodes, {len(A.ICON_REG)} icons, "
              f"{len(controls)} controls")
