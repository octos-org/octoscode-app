#!/usr/bin/env python3
"""Board 5 (design/stage-a/phase4-new5) Stage B — the Memory cards.

Board 5's frames are dynamic dialog states (lists the server fills, a
desktop window shorter than the artboard, a 360 px phone), so — like the
board-3 dialogs since A4 (`screens/board3/ui.rs`: "Why flow, not the measured
card") — each card is the board-3 dialog KIT's surface in that frame's state,
rendered by the REAL app, reached by CLICK from Settings > Capabilities >
Memory against `replay_serve --scenario memory` (faithful replies, no model),
captured by the walks `tools/walk/a36_memory.py` and
`tools/walk/a36_capabilities.py`.

This tool writes, per card, from a walk output tree:

  cards/phase4n5-NN/reference.png   the frame cut from atlas.png (README box)
  cards/phase4n5-NN/card.json       frame, state, how it is reached, ids, methods
  evidence/gate-b/phase4n5-NN-native-<window>.png   the app's capture
  evidence/gate-b/phase4n5-NN-review-<window>.png   reference | native
  evidence/gate-b/phase4n5-NN-snap-<window>.json    its /snap (scrubbed)

Run from the repo root after the walks:
  python3 design/stage-b/phase4-new5/tools/cards.py <memory walk out> <capabilities walk out>
  (each out has desktop/ and phone/ with the walk's phase dirs)
"""
import json
import pathlib
import shutil
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[4]
HERE = pathlib.Path(__file__).resolve().parents[1]
ATLAS = ROOT / "design/stage-a/phase4-new5/atlas.png"
GATE = HERE / "evidence/gate-b"

# Frame -> (README box, title).
FRAMES = {
    1: ((26, 129, 1227, 1044), "Settings · Capabilities (desktop)"),
    2: ((26, 1148, 1227, 2093), "Memory · desktop"),
    3: ((1296, 141, 1670, 1173), "Settings · phone"),
    4: ((1708, 141, 2081, 1173), "Memory · phone"),
    5: ((2118, 141, 2495, 1173), "Search results"),
    6: ((2534, 141, 2905, 1173), "Search result · opened"),
    7: ((1296, 1277, 1662, 2076), "Entity page"),
    8: ((1702, 1277, 2070, 2076), "Long-term memory"),
    9: ((2113, 1277, 2495, 2076), "Add a note"),
    10: ((2534, 1277, 2900, 2080), "Memory · states"),
    11: ((2949, 141, 3327, 1174), "Memory · dark"),
    12: ((3365, 143, 3747, 1204), "Memory · 中文"),
}

# card id -> (frame, state, [(window, walk, phase, capture)], ids, methods, how)
CARDS = {
    "phase4n5-01": (1, "Settings > Capabilities: Skills, MCP servers, Memory",
                    [("desktop", "cap", "main", "01-capabilities-main-desktop")],
                    ["set_nav_capabilities", "sec_capabilities", "set_cap_skills", "set_cap_mcp", "set_cap_memory"],
                    ["profile/skills/list", "mcp/status/list", "memory/overview"],
                    "Settings (header) > Capabilities"),
    "phase4n5-02": (2, "the overview (desktop)",
                    [("desktop", "mem", "main", "01-overview-desktop")],
                    ["b3_mem_title", "b3_mem_scope", "b3_mem_add", "b3_mem_refresh", "b3_mem_query", "b3_mem_lt",
                     "b3_mem_lt_more", "b3_mem_today", "b3_mem_entity_0", "b3_mem_staging"],
                    ["memory/overview"], "Settings > Capabilities > Memory 'Open'"),
    "phase4n5-03": (3, "Settings > Capabilities on the phone (the rail chip)",
                    [("phone", "cap", "main", "01-capabilities-main-phone")],
                    ["set_rail_capabilities", "sec_capabilities", "set_cap_memory"],
                    ["profile/skills/list", "mcp/status/list", "memory/overview"], "Settings > the puzzle rail chip"),
    "phase4n5-04": (4, "the overview (phone sheet)",
                    [("phone", "mem", "main", "01-overview-phone"), ("phone", "mem", "main", "01b-overview-scrolled-phone")],
                    ["b3_mem_title", "b3_mem_add", "b3_mem_lt", "b3_mem_entity_0", "b3_mem_staging"],
                    ["memory/overview"], "Settings > Capabilities > Memory 'Open'"),
    "phase4n5-05": (5, "search results, the kind filter, the untrusted chip",
                    [("phone", "mem", "main", "04-results-phone"), ("desktop", "mem", "main", "04-results-desktop")],
                    ["b3_mem_query", "b3_mem_kind_0", "b3_mem_count", "b3_mem_hit_0", "b3_mem_hit_1_trust"],
                    ["memory/search"], "type 'steer queue' in the search field, Enter"),
    "phase4n5-06": (6, "a hit opened: the record, untrusted content as data",
                    [("phone", "mem", "main", "06-record-phone"), ("desktop", "mem", "main", "06-record-desktop")],
                    ["b3_mem_back", "b3_mem_rec_title", "b3_mem_rec_meta", "b3_mem_rec_body", "b3_mem_rec_trust", "b3_mem_rec_id"],
                    ["memory/load"], "the 'Backoff for redelivery' hit"),
    "phase4n5-07": (7, "an entity page",
                    [("phone", "mem", "main", "03-entity-phone"), ("desktop", "mem", "main", "03-entity-desktop")],
                    ["b3_mem_back", "b3_mem_ent_title", "b3_mem_ent_md"], ["memory/entity"], "the 'steer-queue' entity row"),
    "phase4n5-08": (8, "long-term memory, cut by the server: the notice",
                    [("phone", "mem", "truncated", "10-long-term-truncated-phone"),
                     ("phone", "mem", "main", "02-long-term-phone"), ("desktop", "mem", "main", "02-long-term-desktop")],
                    ["b3_mem_lt_page_title", "b3_mem_lt_full", "b3_mem_lt_cut"], ["memory/overview"], "Show all"),
    "phase4n5-09": (9, "Add a note",
                    [("phone", "mem", "main", "07-add-note-phone"), ("desktop", "mem", "main", "07-add-note-desktop"),
                     ("phone", "mem", "main", "08-added-phone")],
                    ["b3_mem_add_title", "b3_mem_add_note", "b3_mem_add_trust", "b3_mem_add_cancel", "b3_mem_add_submit",
                     "b3_mem_receipt"], ["memory/ingest"], "Add note"),
    "phase4n5-10a": (10, "(a) an empty profile",
                     [("phone", "mem", "empty", "13-empty-phone"), ("desktop", "mem", "empty", "13-empty-desktop")],
                     ["b3_mem_empty"], ["memory/overview"], "open on an empty profile"),
    "phase4n5-10b": (10, "(b) loading",
                     [("phone", "mem", "loading", "14-loading-phone"), ("desktop", "mem", "loading", "14-loading-desktop")],
                     ["b3_mem_loading"], ["memory/overview"], "open while the overview is in flight"),
    "phase4n5-10c": (10, "(c) refused — D1's bounded problem + next step (today's server)",
                     [("phone", "mem", "today", "16-today-search-refused-phone"),
                      ("phone", "mem", "today", "15-today-refused-phone"),
                      ("desktop", "mem", "today", "16-today-search-refused-desktop"),
                      ("phone", "mem", "notrunning", "17-search-not-running-phone")],
                     ["b3_mem_error", "b3_mem_error_detail", "b3_mem_error_next"], ["memory/overview", "memory/search"],
                     "a6ea8505: open, then search"),
    "phase4n5-11": (11, "the overview in the dark theme (D4)",
                    [("phone", "mem", "dark", "19-dark-overview-phone"), ("phone", "mem", "dark", "21-dark-record-phone"),
                     ("desktop", "mem", "dark", "19-dark-overview-desktop")],
                    ["b3_dialog", "b3_mem_lt"], ["memory/overview"], "the app theme set to Dark"),
    "phase4n5-12": (12, "the overview in Chinese (Noto Sans SC)",
                    [("phone", "mem", "zh", "22-zh-overview-phone"), ("phone", "mem", "zh", "23-zh-add-phone"),
                     ("desktop", "mem", "zh", "22-zh-overview-desktop")],
                    ["b3_mem_title", "b3_mem_scope"], ["memory/overview"], "Settings > Preferences > 简体中文"),
}


def scrub(text: str) -> str:
    import re
    users = "/" + "Users" + "/"
    text = re.sub(re.escape(users) + r"[^\s\"']+", "<PATH>", text)
    return re.sub(r"/(private/)?" + "var" + r"/folders/[^\s\"']+", "<TMP>", text)


def review(ref: Image.Image, native: Image.Image) -> Image.Image:
    """reference | native at one height (the native's), on a light grey."""
    h = native.height
    r = ref.resize((max(1, round(ref.width * h / ref.height)), h), Image.LANCZOS)
    out = Image.new("RGB", (r.width + native.width + 24, h), (232, 232, 235))
    out.paste(r, (0, 0))
    out.paste(native.convert("RGB"), (r.width + 24, 0))
    if out.width > 1400:
        out = out.resize((1400, round(out.height * 1400 / out.width)), Image.LANCZOS)
    return out


def main():
    mem_out = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "tmp/a36b/final/memory"
    cap_out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "tmp/a36b/final/capabilities"
    atlas = Image.open(ATLAS)
    GATE.mkdir(parents=True, exist_ok=True)
    report = []
    for card, (frame, state, shots, ids, methods, how) in CARDS.items():
        box, title = FRAMES[frame]
        d = HERE / "cards" / card
        d.mkdir(parents=True, exist_ok=True)
        ref = atlas.crop(box)
        ref.save(d / "reference.png")
        evidence = []
        for window, walk, phase, cap in shots:
            src_root = mem_out if walk == "mem" else cap_out
            src = src_root / window / phase / f"{cap}.png"
            if not src.exists():
                report.append(f"{card}: MISSING {src}")
                continue
            tag = cap.split("-", 1)[1].rsplit("-", 1)[0]
            native = GATE / f"{card}-native-{window}-{tag}.png"
            shutil.copy(src, native)
            review(ref, Image.open(native)).save(GATE / f"{card}-review-{window}-{tag}.png")
            snap = src.with_suffix("").with_suffix(".snap.json")
            snap = src.parent / f"{cap}.snap.json"
            if snap.exists():
                (GATE / f"{card}-snap-{window}-{tag}.json").write_text(scrub(snap.read_text()))
            evidence.append({"window": window, "native": native.name, "review": f"{card}-review-{window}-{tag}.png",
                             "walk": f"tools/walk/a36_{'memory' if walk == 'mem' else 'capabilities'}.py",
                             "phase": phase, "capture": cap})
        (d / "card.json").write_text(json.dumps({
            "card": card, "board": "design/stage-a/phase4-new5", "frame": frame, "frame_title": title,
            "atlas_box": list(box), "state": state, "reached_by": how, "native_ids": ids, "methods": methods,
            "kit": "crates/octoscode-module/src/screens/board3/memory.rs (board-3 dialog kit, ui.rs)",
            "evidence": evidence,
        }, indent=2, ensure_ascii=False) + "\n")
        report.append(f"{card}: frame {frame} ({title}) — {len(evidence)} capture(s)")
    print("\n".join(report))


if __name__ == "__main__":
    main()
