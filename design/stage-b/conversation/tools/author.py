#!/usr/bin/env python3
"""Author the 5 live-gate conversation contracts from the approved stage-a atlas.

Sources of truth (no invention):
  * the approved prompt  design/stage-a/conversation/atlas-prompt.md (declared text, palette, fixture)
  * measured OCR bounds  design/stage-b/conversation/ocr/conversation-NN.ocr.json (Apple Vision, logical 406x776)

Every text node's `text` is the string the atlas actually shows (OCR, with only
obvious glyph-confusion fixed: 'v' -> the model-picker chevron, "l'Il" -> "I'll").
`observe` re-measures x/y/w/h/size from the ink, so the seeds here are just the
authored intent that OCR is checked against. No numerical data is invented.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]          # design/stage-b/conversation
FONT = "self:resources/ux/Inter-{w}.ttf"

C = {  # packed ARGB, from the approved prompt's palette
    "white":  0xFFFFFFFF, "panel": 0xFFF7F7F8, "hairline": 0xFFE5E5E7,
    "ink":    0xFF1D1D1F, "muted": 0xFF6E6E73, "black": 0xFF000000,
    "blue":   0xFF2F6FEB, "green": 0xFF1F883D, "greenbg": 0xFFE6F4EA,
    "red":    0xFFCF222E, "redbg": 0xFFFDECEC, "bubble": 0xFFF2F2F7,
}

def text(id, s, x, y, w, h, *, size=15, weight=400, color="ink"):
    return {"t": "text", "id": id, "text": s, "x": x, "y": y, "w": w, "h": h,
            "size": size, "line_height": h + 2, "weight": weight, "color": C[color],
            "variant": "single_line", "alignx": 0,
            "font_src": FONT.format(w={400: 400, 500: 500, 600: 600, 700: 700}.get(weight, 400))}

def stack(id, x, y, w, h, kids=None, *, bg=None, radius=0):
    n = {"t": "stack", "id": id, "x": x, "y": y, "w": w, "h": h, "c": kids or []}
    if bg: n.update(variant="surface", bg=C[bg], radius=radius)
    return n

def button(id, x, y, w, h, kids=None, *, bg=None, radius=0, enabled=1):
    n = {"t": "button", "id": id, "x": x, "y": y, "w": w, "h": h, "c": kids or [], "enabled": enabled}
    if bg: n.update(bg=C[bg], radius=radius)
    return n

def page(id, title, kids):
    return {"schema_version": 1, "id": id, "app": "octoscode", "number": int(id[-2:]),
            "title": title, "structure": "Native component reconstructed from the approved conversation atlas",
            "artboard": [406, 776], "font_family": "Inter",
            "palette": {"name": "OctosCode", "page": "#FFFFFF", "panel": "#F7F7F8",
                        "ink": "#1D1D1F", "muted": "#6E6E73", "accent": "#2F6FEB"},
            "content_source": "Approved stage-a atlas (design/stage-a/conversation/atlas.png) + measured OCR bounds",
            "graphics": {},
            "tree": stack("page", 0, 0, 406, 776, kids, bg="white")}

# ---------------------------------------------------------------- scene 1: THREAD LIST
S1 = page("conversation-01", "Thread list", [
    text("brand", "OctosCode ▾", 22, 46, 155, 27, size=19, weight=600),
    button("new_chat", 16, 120, 374, 44, [
        text("new_chat_label", "New chat", 36, 133, 88, 23, size=15, weight=500),
    ]),
    stack("thread_list", 16, 212, 374, 420, [
        button("thread_1", 16, 218, 374, 56, [
            text("thread_1_title", "Fix steer queue drop on reconnect", 32, 228, 310, 33, size=15, weight=500),
        ], bg="bubble", radius=10),
        button("thread_2", 16, 302, 374, 56, [
            text("thread_2_title", "Add session fork", 35, 312, 146, 22, size=15),
        ]),
        button("thread_3", 16, 384, 374, 56, [
            text("thread_3_title", "Review PR #2556", 36, 394, 149, 25, size=15),
        ]),
        button("thread_4", 16, 470, 374, 56, [
            text("thread_4_title", "Bump octos-core to abea8505", 36, 480, 259, 26, size=15),
        ]),
        button("thread_5", 16, 554, 374, 56, [
            text("thread_5_title", "Why is hydrate slow?", 35, 564, 180, 27, size=15),
        ]),
    ]),
])

# ------------------------------------------------------------- scene 3: STREAMING TURN
S3 = page("conversation-03", "Streaming turn", [
    stack("user_bubble", 120, 52, 270, 76, [
        text("user_line1", "Fix the steer queue so queued", 105, 60, 256, 28, size=15, weight=500, color="white" if False else "ink"),
        text("user_line2", "steers survive a reconnect", 105, 100, 227, 22, size=15, weight=500, color="ink"),
    ], bg="bubble", radius=16),
    text("activity", "Working · 12s", 64, 194, 123, 27, size=15, weight=500, color="muted"),
    text("assistant_1", "I'm tracing how queued steers are", 21, 255, 276, 25, size=15),
    text("assistant_2", "handled across reconnects…", 23, 292, 230, 21, size=15),
    text("assistant_3", "I'll run tests to confirm the fix", 21, 349, 252, 21, size=15),
    text("assistant_4", "and update the affected code…", 22, 381, 281, 29, size=15),
    stack("context_chips", 29, 500, 342, 40, [
        text("chip_workspace", "octos", 37, 509, 45, 20, size=13, weight=500),
        text("chip_mode", "Local", 129, 508, 45, 21, size=13, weight=500),
        text("chip_branch", "feat/steer-queue", 228, 509, 123, 21, size=13, weight=500),
    ]),
    stack("composer", 16, 584, 374, 150, [
        stack("composer_field", 30, 592, 346, 40, [
            text("composer_placeholder", "Ask Octos anything", 29, 600, 164, 26, size=15, color="muted"),
        ], bg="panel", radius=10),
        text("permission", "Ask for approval", 73, 687, 103, 21, size=13, color="muted"),
        text("model_picker", "v4-flash ▾", 215, 690, 74, 19, size=13, weight=500),
        button("composer_stop", 350, 676, 36, 36, [], bg="black", radius=18),
    ]),
])

# ---------------------------------------------------------------- scene 4: TOOL CELLS
S4 = page("conversation-04", "Tool cells", [
    stack("tool_rows", 16, 60, 374, 250, [
        button("tool_1", 16, 68, 374, 60, [
            text("tool_1_summary", "Read ui_protocol_transport.rs", 88, 77, 232, 23, size=14, weight=500),
            text("tool_1_meta", "· 412 lines", 87, 104, 79, 19, size=13, color="muted"),
        ]),
        button("tool_2", 16, 196, 374, 60, [
            text("tool_2_summary", "Search 'steer_dropped'", 88, 205, 178, 25, size=14, weight=500),
            text("tool_2_meta", "· 7 matches", 87, 234, 89, 21, size=13, color="muted"),
        ]),
        button("tool_3", 16, 326, 374, 60, [
            text("tool_3_cmd1", ">_ Ran cargo test -p octos-cli", 35, 336, 271, 33, size=14, weight=500),
            text("tool_3_cmd2", "steer_queue", 88, 370, 90, 21, size=14, weight=500),
        ]),
    ]),
    stack("tool_3_output", 30, 424, 346, 176, [
        text("out_running", "running 12 tests", 45, 442, 136, 23, size=13, color="muted"),
        text("out_test", "test steer_queue::reconnect_ok ... ok", 39, 515, 303, 24, size=13),
        text("out_result", "test result: ok. 12 passed; 0 failed", 43, 568, 304, 24, size=13, weight=500, color="green"),
    ], bg="panel", radius=12),
])

# ------------------------------------------------------------ scene 8: COMPOSER STATES
S8 = page("conversation-08", "Composer states", [
    stack("composer_idle", 16, 140, 374, 170, [
        stack("idle_field", 30, 150, 346, 40, [
            text("idle_placeholder", "Ask Octos anything", 30, 166, 164, 31, size=15, color="muted"),
        ], bg="panel", radius=10),
        text("idle_permission", "Ask for approval", 77, 266, 103, 22, size=13, color="muted"),
        text("idle_model", "v4-flash ▾", 213, 270, 75, 20, size=13, weight=500),
    ]),
    stack("queued_row", 16, 408, 374, 44, [
        text("queued_text", "1 queued · Steer now · ✕", 30, 427, 210, 24, size=13, weight=500),
    ], bg="panel", radius=10),
    stack("composer_active", 16, 480, 374, 170, [
        text("active_draft", "also add a test for reconnect", 29, 507, 238, 23, size=15),
        text("active_permission", "Ask for approval", 77, 560, 103, 22, size=13, color="muted"),
        text("active_model", "v4-flash ▾", 213, 564, 75, 20, size=13, weight=500),
    ]),
])

# ----------------------------------------------------------- scene 9: COMPLETED ANSWER
S9 = page("conversation-09", "Completed answer", [
    button("worked_row", 16, 32, 374, 44, [
        text("worked_label", "Worked for 3m 4s ›", 26, 42, 181, 23, size=14, weight=500, color="muted"),
    ]),
    text("answer_lead", "Queued steers now survive a reconnect.", 28, 110, 323, 26, size=16, weight=600),
    stack("answer_bullets", 16, 158, 374, 420, [
        text("bullet_1a", "• Fixed loss of queued steers when", 19, 170, 313, 33, size=14),
        text("bullet_1b", "reconnecting after a drop in", 45, 209, 242, 33, size=14),
        text("bullet_1c", "steer_dropped handling.", 55, 251, 219, 33, size=14),
        text("bullet_2a", "• Updated ui_protocol_transport.rs", 23, 319, 342, 33, size=14),
        text("bullet_2b", "to persist queued steers to the", 50, 363, 258, 25, size=14),
        text("bullet_2c", "session ledger.", 51, 403, 128, 26, size=14),
        text("bullet_3", "• All tests pass: 12 passed.", 26, 468, 256, 26, size=14),
        text("bullet_4", "• Changes included in commit a6ea8505.", 23, 532, 364, 33, size=14),
    ]),
    text("answer_timestamp", "Sep 28, 9:41 PM", 266, 664, 122, 26, size=13, color="muted"),
])

SPECS = {s["id"]: s for s in (S1, S3, S4, S8, S9)}

if __name__ == "__main__":
    for id, doc in SPECS.items():
        out = ROOT / "cards" / id / "contract.json"
        out.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
        print("wrote", out)
