#!/usr/bin/env python3
"""A1 — numeric /snap checks for the conversation screens (connect, empty,
running and completed turns), at any module size.

usage: ux_checks.py <snap.json> <connect|conversation>

Reads a `/snap?all=1` dump (the Makepad instrument) and asserts the geometry
the A1 layout promises, with the web's numbers (theme.css:29-33,
styles.css:519/771, the <=760 px media query):

* the transcript column is centred in the conversation pane with >= the
  web's gutter (24 px desktop / 16 px phone) and has the web's width
  (clamp(736, 62vw, 1040) bounded by the pane);
* the composer is the web's chat-wide width (+32 px, bounded by the pane,
  12 px phone gutter) and centred; its text starts on the column's edge;
* no text widget is clipped by its row / card / the module window;
* every hit target is >= 28 px in both axes;
* timeline rows do not overlap.

Prints one PASS/FAIL line per check and a one-line summary (the CSV
`checks` cell). Exit 0 only when every check passed. Never prints input
values (`val`): a TextInput's raw value can be a secret.
"""
import json
import re
import sys

TOL = 1.5


def load(path):
    d = json.load(open(path))
    rows = []
    for w in d.get("s", []):
        r = w.get("r") or [0, 0, 0, 0]
        if r[2] <= 0 or r[3] <= 0:
            continue
        rows.append({"i": str(w.get("i") or ""), "ty": w.get("ty"), "r": r, "t": w.get("t") or ""})
    return rows


def find(rows, wid):
    for w in rows:
        if w["i"] == wid:
            return w
    return None


def right(r):
    return r[0] + r[2]


def bottom(r):
    return r[1] + r[3]


def inside(inner, outer, tol=TOL):
    return (inner[0] >= outer[0] - tol and right(inner) <= right(outer) + tol
            and inner[1] >= outer[1] - tol and bottom(inner) <= bottom(outer) + tol)


def metrics(window_w, pane_w):
    phone = window_w < 760
    pad, cpad = (16.0, 12.0) if phone else (24.0, 24.0)
    chat = min(max(0.62 * window_w, 736.0), 1040.0)
    column = int(min(chat, pane_w - 2 * pad))
    composer = int(min(chat + 32.0, pane_w - 2 * cpad))
    return phone, pad, cpad, column, composer


class Report:
    def __init__(self):
        self.lines = []
        self.ok = 0
        self.n = 0
        self.summary = []

    def check(self, name, cond, detail, short=None):
        self.n += 1
        self.ok += bool(cond)
        self.lines.append(f"{'PASS' if cond else 'FAIL'} {name}: {detail}")
        if short:
            self.summary.append(short if cond else f"FAIL[{short}]")

    def done(self):
        for l in self.lines:
            print(l)
        print(f"SUMMARY {self.ok}/{self.n} pass; " + "; ".join(self.summary))
        return 0 if self.ok == self.n else 1


def conversation(rows, rep):
    module = find(rows, "OctoscodeView") or next((w for w in rows if w["ty"] == "OctoscodeView"), None)
    if module is None:
        module = next(w for w in rows if w["ty"] == "MpModuleView")
    pane = find(rows, "conversation_column")
    mod_r, pane_r = module["r"], pane["r"]
    # The live chrome root: narrower than the module under an
    # OCTOSENSE_WINDOW_SIZE viewport cap (the root's right padding).
    base = find(rows, "base")
    view_w = base["r"][2] if base else mod_r[2]
    phone, pad, cpad, col_w, comp_w = metrics(view_w, pane_r[2])
    # The rows' content boxes: full-width rows (worked-for, tool cards,
    # actions) and the right-aligned bubble mark the column's two edges.
    kinds = ("workedfor", "toolcell", "assistantprose", "answeractions", "userbubble", "workingrow")
    items = [w for w in rows if re.fullmatch(r"i\d+_(%s)" % "|".join(kinds), w["i"])]
    full = [w for w in items if re.search(r"_(workedfor|toolcell|workingrow)$", w["i"])]
    if full:
        left = min(w["r"][0] for w in full)
        width = max(w["r"][2] for w in full)
        gl = left - pane_r[0]
        gr = right(pane_r) - (left + width)
        rep.check("column width", abs(width - col_w) <= TOL,
                  f"{width:.0f} vs the web's {col_w} (pane {pane_r[2]:.0f}, module {mod_r[2]:.0f})",
                  f"column {width:.0f}={col_w}")
        rep.check("column centred", abs(gl - gr) <= TOL and gl >= pad - TOL,
                  f"gutters {gl:.0f}/{gr:.0f} (min {pad:.0f})", f"gutters {gl:.0f}/{gr:.0f}")
        col_l, col_r = left, left + width
    else:
        col_l = pane_r[0] + max(pad, (pane_r[2] - col_w) / 2)
        col_r = col_l + col_w
    for b in (w for w in items if w["i"].endswith("_userbubble")):
        rep.check(f"{b['i']} right-aligned", abs(right(b["r"]) - col_r) <= TOL,
                  f"bubble right {right(b['r']):.0f} vs column right {col_r:.0f}")
        cap = min(0.82 * col_w, 680)
        rep.check(f"{b['i']} capped", b["r"][2] <= cap + TOL, f"{b['r'][2]:.0f} <= min(680, 82%)={cap:.0f}")
        lab = find(rows, b["i"] + "_0")
        if lab:
            rep.check(f"{b['i']} text inside", inside(lab["r"], b["r"]),
                      f"label {lab['r']} in bubble {b['r']}")
    for p in (w for w in items if w["i"].endswith("_assistantprose")):
        rep.check(f"{p['i']} prose width", p["r"][2] <= min(col_w, 700 if not phone else col_w) + TOL
                  and p["r"][0] >= col_l - TOL,
                  f"{p['r'][2]:.0f} wide at x={p['r'][0]:.0f} (column {col_l:.0f}..{col_r:.0f})")
    comp = find(rows, "i0_composer")
    if comp:
        cl, cw = comp["r"][0], comp["r"][2]
        rep.check("composer width", abs(cw - comp_w) <= TOL, f"{cw:.0f} vs the web's {comp_w}",
                  f"composer {cw:.0f}={comp_w}")
        gl = cl - pane_r[0]
        gr = right(pane_r) - right(comp["r"])
        rep.check("composer centred", abs(gl - gr) <= TOL and gl >= cpad - TOL,
                  f"gutters {gl:.0f}/{gr:.0f} (min {cpad:.0f})")
        inp = find(rows, "i0_composer_0")
        if inp and full:
            dx = inp["r"][0] - col_l
            want = 16 - (cw - (col_r - col_l)) / 2
            rep.check("composer text on the column edge", abs(dx - want) <= TOL,
                      f"input x - column x = {dx:.0f} (card padding 16, card {cw - (col_r - col_l):.0f} px wider)",
                      f"composer text dx {dx:.0f}")
        rep.check("composer inside the module", inside(comp["r"], mod_r),
                  f"composer bottom {bottom(comp['r']):.0f} <= module bottom {bottom(mod_r):.0f}")
    # Clipping: every text widget of the conversation sits inside the pane
    # and the module window (and above the composer for timeline text).
    texts = [w for w in rows if w["ty"] in ("Label", "TextInput", "Markdown")
             and pane_r[0] - 1 <= w["r"][0] <= right(pane_r) + 1
             and pane_r[1] - 1 <= w["r"][1] <= bottom(pane_r)]
    clipped = [w["i"] for w in texts if not inside(w["r"], pane_r) or not inside(w["r"], mod_r)]
    rep.check("no clipped text", not clipped, f"{len(texts)} text widgets inside pane+module; out: {clipped[:6]}",
              f"text {len(texts)}/{len(texts) - len(clipped)} inside")
    # Hit targets >= 28 px (a row cut by the scrolled list's own edge is a
    # viewport clip, not a small target: skip rows touching the list edges).
    lst = find(rows, "timeline_list")
    def cut(w):
        if lst is None or w["i"] not in ("row_hit", "tool_hit", "worked_hit", "answer_copy_hit"):
            return False
        return w["r"][1] <= lst["r"][1] + 1 or bottom(w["r"]) >= bottom(lst["r"]) - 1
    hits = [w for w in rows if w["ty"] == "Button" and not cut(w) and (
        w["i"] in ("send_hit", "mic_hit", "plus_hit", "approval_pill_hit", "answer_copy_hit", "row_hit",
                   "tool_hit", "worked_hit"))]
    small = [(w["i"], w["r"][2], w["r"][3]) for w in hits if w["r"][2] < 28 - 0.5 or w["r"][3] < 28 - 0.5]
    rep.check("controls >= 28 px", not small, f"{len(hits)} hit targets; too small: {small}",
              f"{len(hits)} controls>=28")
    # Rows do not overlap.
    boxes = sorted((w["r"] for w in items if not w["i"].endswith("_userbubble")), key=lambda r: r[1])
    boxes += [w["r"] for w in items if w["i"].endswith("_userbubble")]
    boxes.sort(key=lambda r: r[1])
    over = [(a, b) for a, b in zip(boxes, boxes[1:]) if bottom(a) > b[1] + 0.5]
    rep.check("rows do not overlap", not over, f"{len(boxes)} rows; overlaps {over[:2]}", f"{len(boxes)} rows no overlap")


def connect(rows, rep):
    module = next(w for w in rows if w["ty"] == "MpModuleView")
    mod_r = module["r"]
    # Under an OCTOSENSE_WINDOW_SIZE cap the first-run chrome is narrower
    # than the module (the root's right padding).
    first = find(rows, "first_run")
    view_r = first["r"] if first else mod_r
    phone = view_r[2] < 760
    sidebar = 0 if phone else 261
    card = find(rows, "connect_card")
    pane_l, pane_r = view_r[0] + sidebar, right(view_r)
    c = card["r"]
    gl, gr = c[0] - pane_l, pane_r - right(c)
    rep.check("card centred in the pane", abs(gl - gr) <= TOL + 16 and gl >= 16 - TOL,
              f"gutters {gl:.0f}/{gr:.0f} (pane right of the {sidebar} px sidebar; card padding-left adds 16)",
              f"card gutters {gl:.0f}/{gr:.0f}")
    rep.check("card width", c[2] <= 480 + TOL, f"{c[2]:.0f} <= 480 (board 4 frame 4)", f"card {c[2]:.0f}<=480")
    rep.check("card inside the module", inside(c, mod_r), f"card {c} in module {mod_r}", "card inside")
    texts = [w for w in rows if w["i"].startswith("connect_") and w["ty"] in ("Label", "TextInput")]
    out = [w["i"] for w in texts if not inside(w["r"], c)]
    rep.check("no clipped text", not out, f"{len(texts)} card texts inside the card; out: {out}",
              f"text {len(texts)}/{len(texts) - len(out)} inside")
    hits = [w for w in rows if w["ty"] == "Button" and w["i"] in ("connect_btn", "connect_solo", "connect_eye")]
    small = [(w["i"], w["r"][2], w["r"][3]) for w in hits if w["r"][2] < 27.5 or w["r"][3] < 27.5]
    rep.check("controls >= 28 px", not small and len(hits) == 3, f"{[(w['i'], w['r'][2], w['r'][3]) for w in hits]}",
              f"{len(hits)} controls>=28")
    field_s = find(rows, "connect_server")
    field_t = find(rows, "connect_token")
    if field_s and field_t:
        rep.check("fields share one left edge", abs(field_s["r"][0] - field_t["r"][0]) <= TOL,
                  f"{field_s['r'][0]:.0f} / {field_t['r'][0]:.0f}")


def main():
    path, mode = sys.argv[1], sys.argv[2]
    rows = load(path)
    rep = Report()
    if mode == "connect":
        connect(rows, rep)
    else:
        conversation(rows, rep)
    sys.exit(rep.done())


if __name__ == "__main__":
    main()
