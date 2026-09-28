#!/usr/bin/env python3
"""Author the 12 conversation contracts from the approved atlas.

Text is **measured** (Apple Vision OCR on each scene's reference.png, logical
406x776) and corrected only where the approved prompt (atlas-prompt.md) names a
different string — OCR glyph confusions, not invented copy. Positions are the
measured OCR bounds. Structure (which lines are buttons/inputs) comes from the
approved prompt. No numerical data is invented.

Run:  python3 tools/author_all.py
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OCR = ROOT / "ocr"

C = {"white": 0xFFFFFFFF, "panel": 0xFFF7F7F8, "hairline": 0xFFE5E5E7,
     "ink": 0xFF1D1D1F, "muted": 0xFF6E6E73, "black": 0xFF000000,
     "blue": 0xFF2F6FEB, "green": 0xFF1F883D, "red": 0xFFCF222E,
     "bubble": 0xFFF2F2F7}
FONT = "self:resources/ux/Inter-{w}.ttf"

# Corrected copy per the approved prompt, keyed on the OCR string.
FIX = {
    "Fix steer queue drop on reconnect": "Fix steer queue drop on reconnect",
    "Add session fork": "Add session fork",
    "Review PR #2556": "Review PR #2566",
    "Bump octos-core to abea8505": "Bump octos-core to a6ea8505",
    "Why is hydrate slow?": "Why is hydrate slow?",
    "v4-flash v": "v4-flash ▾",
    "V4-flash v": "v4-flash ▾",
    "handled across reconnects...": "handled across reconnects…",
    "l'Il run tests to confirm the fix": "I'll run tests to confirm the fix",
    "and update the affected code... |": "and update the affected code…",
    "and update the affected code...": "and update the affected code…",
    "1 queued • Steer now • X": "1 queued · Steer now · ✕",
    "Sep 28,9:41 PM": "Sep 28, 9:41 PM",
    "Worked for 3m 4s ›": "Worked for 3m 4s ›",
    "Updated ui_protocol_transport.rs": "• Updated ui_protocol_transport.rs",
}
# Scene titles (from the approved prompt).
TITLES = {1: "Thread list", 2: "New chat", 3: "Streaming turn", 4: "Tool cells",
          5: "Inline approval", 6: "User question", 7: "Edited files",
          8: "Composer states", 9: "Completed answer", 10: "Goal and plan",
          11: "Review diff", 12: "Settings card"}

# Per-scene role overrides: OCR substring -> (kind, node_id). Bare button/input
# nodes; their label text becomes a child so the copy stays a native Label.
ROLES = {
    1: [("New chat", "new_chat"),
        ("Fix steer queue", "thread_1"),
        ("Add session fork", "thread_2"),
        ("Review PR #25", "thread_3"),
        ("Bump octos-core", "thread_4"),
        ("Why is hydrate", "thread_5")],
    3: [],
    4: [("Read ui_protocol_transport", "tool_1"),
        ("Search 'steer_dropped'", "tool_2"),
        ("Ran cargo test", "tool_3")],
    5: [("Approve once", "approve_once"), ("Approve for session", "approve_session"),
        ("Deny", "deny")],
    6: [("Submit answer", "submit_answer"), ("Skip", "skip")],
    7: [("Undo", "undo"), ("Review", "review")],
    8: [],
    9: [("Worked for 3m 4s ›", "worked_row")],
    10: [],
    11: [],
    12: [],
}
# Inputs (kind 'input') per scene: OCR substring -> node id (placeholder text).
INPUTS = {
    6: [("Add a note", "note_input")],
    8: [("Ask Octos anything", "composer_input")],
    3: [("Ask Octos anything", "composer_input")],
}
# Muted (secondary grey) text, by exact corrected string.
MUTED = {"• 412 lines", "• 7 matches", "Working · 12s", "Ask for approval",
         "running 12 tests", "Sep 28, 9:41 PM", "Add a note", "Ask Octos anything",
         "+", "••", "Skip", "Deny", }


def txt(id, s, x, y, w, h, *, size, weight=400, color="ink"):
    h2 = round(max(h, size * 1.5), 2)
    return {"t": "text", "id": id, "text": s, "x": round(x, 2), "y": round(y, 2),
            "w": round(max(w, 8), 2), "h": h2, "size": size,
            "line_height": h2, "weight": weight,
            "color": C[color], "variant": "single_line", "alignx": 0,
            "font_src": FONT.format(w=weight if weight in (400, 500, 600, 700) else 400)}


def node(kind, id, x, y, w, h, kids=None, **kw):
    n = {"t": kind, "id": id, "x": round(x, 2), "y": round(y, 2),
         "w": round(w, 2), "h": round(h, 2), "c": kids or []}
    n.update(kw)
    return n


def build(num):
    sid = f"conversation-{num:02d}"
    obs = json.loads((OCR / f"{sid}.ocr.json").read_text())
    w0, h0 = obs["width"], obs["height"]
    sx, sy = 406 / w0, 776 / h0
    roles = dict(ROLES.get(num, []))
    inputs = dict(INPUTS.get(num, []))
    used_role, used_input = set(), set()
    kids = []
    for i, o in enumerate(obs["observations"], 1):
        raw = o["text"]
        s = FIX.get(raw, raw)
        x, y, w, h = o["bounds"][0] * sx, o["bounds"][1] * sy, o["bounds"][2] * sx, o["bounds"][3] * sy
        size = round(min(max(h * 0.72, 11), 22), 2)
        weight = 600 if i == 1 or s.endswith("›") else 400
        color = "muted" if s in MUTED else "ink"
        t = txt(f"t{i:02d}", s, x, y, w, h, size=size, weight=weight, color=color)
        for pat, nid in roles.items():
            if nid in used_role:
                continue
            if pat.lower().replace("'", "") in raw.lower().replace("'", ""):
                used_role.add(nid)
                pad = 8
                kids.append(node("button", nid, x - pad, y - pad, w + 2 * pad, h + 2 * pad,
                                 [dict(t, x=x, y=y)], enabled=1))
                break
        else:
            for pat, nid in inputs.items():
                if nid in used_input:
                    continue
                if pat.lower() in raw.lower():
                    used_input.add(nid)
                    kids.append(node("input", nid, x - 8, y - 8, max(w + 16, 200), h + 16,
                                     [dict(t, x=x, y=y)], enabled=1, placeholder=s))
                    break
            else:
                kids.append(t)
    doc = {"schema_version": 1, "id": sid, "app": "octoscode", "number": num,
           "title": TITLES[num],
           "structure": "Native component reconstructed from the approved conversation atlas",
           "artboard": [406, 776], "font_family": "Inter",
           "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                       "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
           "content_source": "Approved stage-a atlas + measured Apple Vision OCR bounds",
           "graphics": {},
           "tree": node("stack", "page", 0, 0, 406, 776, kids, variant="surface", bg=C["white"])}
    return doc


if __name__ == "__main__":
    for n in range(1, 13):
        out = ROOT / "cards" / f"conversation-{n:02d}" / "contract.json"
        out.write_text(json.dumps(build(n), indent=2, ensure_ascii=False) + "\n")
        print("wrote", out)
