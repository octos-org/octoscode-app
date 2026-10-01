#!/usr/bin/env python3
"""D3b — author board-3 screens 5-8 (inspector, thinking effort, resume
candidates, session strip). Lane-suffixed sibling of author.py (the LESSONS
one-board-folder rule): imports its helpers/palette/Scene unchanged, authors
ROW 2 of the measured hairline grid, writes cards phase4n3-05..08.

Screens (operator-approved atlas-prompt.md, 2026-10-01):
  05 inspector  06 thinking effort  07 resume candidates  08 session strip

Measured atlas grid rows: y=1126-2019 (row 2); columns as board 3's row 1.

Run FROM THE REPO ROOT:
  python3 design/stage-b/phase4-new3/tools/author_5_8.py
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import author as A  # the board-3 author: helpers, palette, Scene, writers

ROOT = A.ROOT
C = A.C
ARTW, ARTH = A.ARTW, A.ARTH

TITLES = {5: "Thread", 6: "Thinking effort", 7: "Resume chat", 8: "Session strip"}
SOURCE = {n: f"phase4-new3-screen-{n:02d}" for n in range(5, 9)}
ATLAS_BOXES = {5: (33, 1126, 496, 2019), 6: (526, 1126, 1000, 2019),
               7: (1027, 1126, 1495, 2019), 8: (1516, 1126, 2005, 2019)}


def toggle(iid, x, y, *, on=True):
    """A blue toggle (blue is the palette's toggle colour) with a real
    t:\"button\" node — taps attach only to button nodes."""
    knob = x + 21 if on else x + 3
    kids = [A.surface(f"{iid}_bg", x, y, 44, 26,
                      bg=("blue" if on else "disabled"), radius=13),
            A.surface(f"{iid}_knob", knob, y + 3, 20, 20, bg="white", radius=10),
            {"t": "button", "id": iid, "x": x, "y": y, "w": 44, "h": 26,
             "enabled": 1}]
    return {"t": "stack", "id": f"{iid}_row", "x": x, "y": y, "w": 44, "h": 26,
            "c": kids}


def seg(iid, label, x, y, w, *, selected=False):
    """One segment of the effort control; the selected one is a white knob
    with ink text, the rest muted labels on the sel track."""
    if selected:
        kids = [A.surface(f"{iid}_bg", x + 2, y + 2, w - 4, 32, bg="white",
                          radius=8),
                A.text(f"{iid}_t", label, x, y + 10, w, 16, weight=600,
                       size=13, alignx=0),
                {"t": "button", "id": iid, "x": x, "y": y, "w": w, "h": 36,
                 "enabled": 1}]
        return {"t": "stack", "id": f"{iid}_row", "x": x, "y": y, "w": w,
                "h": 36, "c": kids}
    return {"t": "stack", "id": f"{iid}_row", "x": x, "y": y, "w": w, "h": 36,
            "c": [A.text(f"{iid}_t", label, x + 8, y + 10, w - 16, 16,
                         color="muted", size=13),
                  {"t": "button", "id": iid, "x": x, "y": y, "w": w, "h": 36,
                   "enabled": 1}]}


def summary_row(sc, iid, chevron, text, meta, y, expanded=False):
    r = A.surface(f"{iid}_row", 24, y, 342, 40, bg="white", radius=10, border=1)
    r["c"] = [A.text(f"{iid}_chev", chevron, 34, y + 11, 16, 16, color="muted",
                     size=13),
              A.text(f"{iid}_t", text, 56, y + 11, 190, 16, color="muted",
                     size=13),
              A.text(f"{iid}_meta", meta, 240, y + 12, 114, 14, color="muted",
                     size=11, alignx=1),
              {"t": "button", "id": iid, "x": 24, "y": y, "w": 342, "h": 40,
               "enabled": 1}]
    sc.put(r)
    sc.observe(text, 56, y + 11, 190, 16)
    sc.control(iid, f"thinking.{iid}", (24, y, 342, 40))


# ---------------------------------------------------------------- screen 05
def build_05(sc):
    scrim, card = A.modal(y=64, h=648)
    sc.put(scrim)
    k = [A.text("h_slash", "/thread", 32, 84, 90, 17, weight=500, size=13,
                mono=True),
         A.text("t_title", "Thread", 32, 108, 200, 24, weight=600, size=20),
         A.text("t_scope", "/home/user/octos · main", 32, 138, 260, 15,
                color="muted", size=12, mono=True)]
    k.append(A.link("inspector_refresh", "↻ Refresh", 282, 86, 92))
    sc.control("inspector_refresh", "inspector_refresh", (282, 86, 92, 20))
    k.append(A.divider("d0", 166))
    k.append(A.text("h_graph", "THREAD GRAPH", 32, 180, 140, 12, color="muted",
                    size=11, weight=600))
    y = 200
    for i, (role, name, depth) in enumerate([
        ("current", "Fix steer queue drop on reconnect", "0"),
        ("parent", "Add session fork", "1"),
        ("parent", "Review PR #2566", "1"),
        ("parent", "Bump octos-core to a6ea8505", "2"),
    ]):
        r = A.surface(f"node_{i}", 32, y, 342, 44, bg="white", radius=10,
                      border=1)
        r["c"] = [A.text(f"node_{i}_role", role, 44, y + 6, 70, 14,
                         color="muted", size=11, mono=True),
                  A.text(f"node_{i}_name", name, 44, y + 24, 250, 15,
                         size=12, weight=500),
                  A.text(f"node_{i}_depth", f"depth {depth}", 290, y + 14, 72,
                         14, color="muted", size=11, alignx=1)]
        k.append(r)
        sc.observe(name, 44, y + 24, 250, 15)
        y += 50
    k.append(A.divider("d1", y + 4))
    y += 16
    k.append(A.text("h_scopes", "APPROVAL SCOPES", 32, y, 160, 12,
                    color="muted", size=11, weight=600))
    y += 20
    for i, scope in enumerate(["Session", "Workspace"]):
        r = A.surface(f"scope_{i}", 32, y, 342, 40, bg="white", radius=10,
                      border=1)
        r["c"] = [A.text(f"scope_{i}_t", scope, 44, y + 11, 160, 16, size=13),
                  A.pill(f"scope_{i}_tag", "allowed", 316, y + 10, color="chip",
                         fg="blue", size=10)]
        k.append(r)
        sc.observe(scope, 44, y + 11, 160, 16)
        y += 46
    k.append(A.divider("d2", y + 4))
    y += 18
    k.append(A.btn("inspector_copy_link", "Copy link", 32, y, 120, 36))
    sc.control("inspector_copy_link", "inspector_copy_link", (32, y, 120, 36))
    k.append(A.text("t_link", "octos://session/dsflash:main", 32, y + 52, 300,
                    15, color="muted", size=12, mono=True))
    sc.observe("octos://session/dsflash:main", 32, y + 52, 300, 15)
    k.append(A.btn("inspector_refresh_bottom", "Refresh", 264, 640, 110, 36,
                   fill="chip", color="ink"))
    sc.control("inspector_refresh_bottom", "inspector_refresh",
               (264, 640, 110, 36))
    card["c"] = k
    sc.put(card)


# ---------------------------------------------------------------- screen 06
def build_06(sc):
    k = [A.text("t_title", "Thinking effort", 24, 28, 240, 22, weight=600,
                size=18)]
    k.append(A.surface("seg_track", 24, 58, 342, 36, bg="sel", radius=10))
    for i, (lab, sel) in enumerate([("Low", False), ("Medium", False),
                                    ("High", True), ("Max", False)]):
        k.append(seg(f"effort_{lab.lower()}", lab, 24 + i * 85.5, 58, 85.5,
                     selected=sel))
        sc.control(f"effort_{lab.lower()}", f"thinking.effort.{lab.lower()}",
                   (24 + i * 85.5, 58, 85.5, 36))
    k.append(A.text("t_help", "Sets how much the model thinks before answering",
                    24, 102, 320, 15, color="muted", size=12))
    sc.observe("Sets how much the model thinks before answering", 24, 102,
               320, 15)
    k.append(A.text("t_show", "Show reasoning", 24, 136, 220, 17, size=14))
    k.append(toggle("toggle_show_reasoning", 322, 132, on=True))
    sc.control("toggle_show_reasoning", "thinking.show_reasoning",
               (322, 132, 44, 26))
    k.append(A.text("t_show_help", "Shows the model's reasoning while it works",
                    24, 160, 300, 14, color="muted", size=11))
    k.append(A.text("t_default", "Default on for new chats", 24, 196, 220, 17,
                    size=14))
    k.append(toggle("toggle_default_new", 322, 192, on=True))
    sc.control("toggle_default_new", "thinking.default_new", (322, 192, 44, 26))
    k.append(A.divider("d0", 240))
    k.append(A.link("thinking_expand_all", "Expand all", 24, 252, 70,
                    color="blue"))
    sc.control("thinking_expand_all", "thinking_expand_all", (24, 252, 70, 20))
    k.append(A.link("thinking_collapse_all", "Collapse all", 104, 252, 80,
                    color="blue"))
    sc.control("thinking_collapse_all", "thinking_collapse_all",
               (104, 252, 80, 20))
    summary_row(sc, "row_0", "▾", "Weighed two approaches", "1,204 tok · 3.2s",
                284, expanded=True)
    lines = ["- Latency vs correctness on the retry path",
             "- Token budget for the wider context",
             "- Order of the two tool calls",
             "- Fallback when the probe times out"]
    for i, line in enumerate(lines):
        k2 = A.text(f"row_0_l{i}", line, 72, 330 + i * 22, 280, 14,
                    color="muted", size=11)
        sc.kids.append(k2)
        sc.observe(line, 72, 330 + i * 22, 280, 14)
    summary_row(sc, "row_1", "▸", "Checked the retry path", "842 tok · 1.9s",
                428)
    # The panel chrome (title/segments/toggles/links) MUST enter the scene:
    # v2/v3 lost it to a dead extend([]) stub (the whole upper half missing).
    sc.kids = k + sc.kids


# ---------------------------------------------------------------- screen 07
def build_07(sc):
    scrim, card = A.modal(y=70, h=640)
    sc.put(scrim)
    k = [A.text("t_title", "Resume chat", 32, 90, 220, 24, weight=600,
                size=20)]
    k.append(A.surface("banner", 32, 124, 342, 56, bg="hl", radius=10))
    k[-1]["c"] = [A.text("banner_t", "History browsing authority changed.",
                         44, 132, 300, 16, weight=500, size=13),
                  A.text("banner_s", "Refresh the catalog before selecting this row.",
                         44, 152, 300, 14, color="muted", size=11)]
    sc.observe("History browsing authority changed.", 44, 132, 300, 16)
    y = 196
    for i, (title, tag, when) in enumerate([
        ("Fix steer queue drop on reconnect", "octos", "2m"),
        ("Add session fork", "octoscode-app", "1h"),
        ("Review PR #2566", "octos", "Yesterday"),
    ]):
        r = A.surface(f"cand_{i}", 32, y, 342, 56, bg="white", radius=10,
                      border=1)
        r["c"] = [A.text(f"cand_{i}_t", title, 44, y + 9, 240, 16, size=13,
                         weight=500),
                  A.text(f"cand_{i}_s", f"{tag} · {when}", 44, y + 30, 200, 14,
                         color="muted", size=11),
                  A.pill(f"cand_{i}_chip", "unverified", 292, y + 8,
                         color="hl", fg="amber", size=10),
                  {"t": "button", "id": f"resume_row_{i}", "x": 32, "y": y,
                   "w": 342, "h": 56, "enabled": 1}]
        k.append(r)
        sc.observe(title, 44, y + 9, 240, 16)
        sc.control(f"resume_row_{i}", f"resume.select.{i}", (32, y, 342, 56))
        y += 62
    k.append(A.surface("confirm", 32, y + 8, 342, 104, bg="sel", radius=10))
    k[-1]["c"] = [A.text("confirm_h", "Confirm exact title to resume:", 44,
                         y + 16, 240, 14, color="muted", size=10, weight=600),
                  A.field("resume_input", "Type the exact thread title above",
                          44, y + 38, 236, 34, color="disabled"),
                  A.btn("resume_confirm", "Resume", 292, y + 62, 74, 32,
                        enabled=False)]
    sc.observe("Type the exact thread title above", 44, y + 38, 236, 34)
    sc.control("resume_confirm", "resume_confirm", (292, y + 62, 74, 32))
    k.append(A.text("t_note", "A confirmed source Session is required to browse history.",
                    32, y + 126, 342, 14, color="muted", size=11))
    sc.observe("A confirmed source Session is required to browse history.",
               32, y + 126, 342, 14)
    lr = A.surface("locked", 32, y + 152, 342, 44, bg="sel", radius=10)
    lr["c"] = [A.text("locked_t", "This retained Session is closed.", 44,
                      y + 165, 240, 16, color="muted", size=12),
               A.pill("locked_chip", "locked", 316, y + 163, color="disabled",
                      fg="white", size=10)]
    k.append(lr)
    sc.observe("This retained Session is closed.", 44, y + 121, 240, 16)
    card["c"] = k
    sc.put(card)


# ---------------------------------------------------------------- screen 08
def build_08(sc):
    strip = A.surface("strip", 0, 0, ARTW, 64, bg="panel", radius=0)
    strip["c"] = [
        A.text("strip_model_t", "glm-4.6", 16, 24, 110, 16, size=13,
               weight=500, mono=True),
        {"t": "button", "id": "strip_model", "x": 0, "y": 0, "w": 135, "h": 64,
         "enabled": 1},
        A.surface("strip_d1", 135, 12, 1, 40, bg="hair", radius=0),
        A.text("strip_activity_t", "Working…", 143, 24, 118, 16, size=13),
        {"t": "button", "id": "strip_activity", "x": 136, "y": 0, "w": 133,
         "h": 64, "enabled": 1},
        A.surface("strip_d2", 270, 12, 1, 40, bg="hair", radius=0),
        A.text("strip_perm_t", "Write", 286, 24, 100, 16, size=13),
        {"t": "button", "id": "strip_permissions", "x": 271, "y": 0, "w": 135,
         "h": 64, "enabled": 1},
    ]
    sc.kids.append(strip)
    sc.observe("glm-4.6", 16, 24, 110, 16)
    sc.observe("Working…", 143, 24, 118, 16)
    sc.observe("Write", 286, 24, 100, 16)
    sc.control("strip_model", "strip_model", (0, 0, 135, 64))
    sc.control("strip_activity", "strip_activity", (136, 0, 133, 64))
    sc.control("strip_permissions", "strip_permissions", (271, 0, 135, 64))
    k = [A.text("t_legend", "Model, permissions, sandbox", 16, 80, 240, 14,
                color="muted", size=11)]
    sc.kids.extend(k)
    sc.observe("Model, permissions, sandbox", 16, 80, 240, 14)
    for i, s in enumerate(["Reconnecting", "Resuming chat…",
                           "Handing back control…"]):
        sc.kids.append(A.pill(f"state_{i}", s, 16, 108 + i * 34, color="chip",
                              fg="muted", size=10))
    ir = A.surface("info", 16, 232, 342, 40, bg="white", radius=10, border=1)
    ir["c"] = [A.surface("info_dot", 28, 161, 10, 10, bg="amber", radius=5),
               A.text("info_t", "Another app is using this session", 48, 156,
                      300, 16, size=12)]
    sc.kids.append(ir)
    sc.observe("Another app is using this session", 48, 156, 300, 16)


BUILDERS = {5: build_05, 6: build_06, 7: build_07, 8: build_08}


def build(num):
    sc = A.Scene(num)
    BUILDERS[num](sc)
    doc = {
        "schema_version": 1, "id": f"phase4n3-{num:02d}", "app": "octoscode",
        "number": num, "title": TITLES[num],
        "structure": "Native component reconstructed from the approved board-3 atlas",
        "artboard": [ARTW, ARTH],
        "artboard_pixels_per_pt": 1.0,
        "content_source": ("Approved stage-a phase4-new3 atlas; text/bounds measured by "
                           "the lane model on the measured hairline-grid crops "
                           "(labeled 'lane-vision'; no external OCR exists for board 3)"),
        "font_family": "Inter",
        "palette": {"name": "OctosCode", "page": A.C_HEX["white"],
                    "panel": A.C_HEX["panel"], "ink": A.C_HEX["ink"],
                    "muted": A.C_HEX["muted"], "accent": A.C_HEX["blue"]},
        "colors": {k: A.C_HEX[k] for k in ("white", "panel", "hair", "ink",
                                           "muted", "black", "blue", "amber",
                                           "green", "red", "sel", "chip",
                                           "disabled")},
        "graphics": {},
        "widgets": [],
        "tree": {"t": "stack", "id": "page", "x": 0, "y": 0, "w": ARTW,
                 "h": ARTH, "variant": "surface", "bg": C["white"], "radius": 0,
                 "c": sc.kids},
    }
    return doc, sc


def main():
    only = [int(x) for x in sys.argv[1:]] or [5, 6, 7, 8]
    for n in only:
        doc, sc = build(n)
        d = ROOT / "cards" / f"phase4n3-{n:02d}"
        (d / "assets").mkdir(parents=True, exist_ok=True)
        (d / "contract.json").write_text(json.dumps(doc, indent=2,
                                                    ensure_ascii=False) + "\n")
        obs = {"width": ARTW, "height": ARTH, "observations": sc.obs,
               "source": "lane-vision on the measured atlas crops (no external OCR for board 3)"}
        (d / "observations.json").write_text(json.dumps(obs, indent=2,
                                                        ensure_ascii=False) + "\n")
        ref = ROOT.parent.parent / "stage-a" / "phase4-new3"
        from PIL import Image
        Image.open(ref / "atlas.png").crop(ATLAS_BOXES[n]).save(d / "reference.png")
        (d / "service-actions.json").write_text(json.dumps(
            {"frame_id": n, "controls": sc.controls, "source": SOURCE[n]},
            indent=2) + "\n")
        print(f"wrote phase4n3-{n:02d}: {len(sc.kids)} top nodes, "
              f"{len(sc.controls)} controls, reference.png")


if __name__ == "__main__":
    main()
