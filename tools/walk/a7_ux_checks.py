#!/usr/bin/env python3
"""A7 — numeric /snap checks for the A7 surfaces (the composer extras, the
answer's code blocks and math, the transcript notices, the history and image
dialogs), on the `/snap?all=1` dumps `tools/walk/a7_composer_walk.py` saves
next to each capture (`A7_WALK_OUT`; input values are stripped there).

usage: a7_ux_checks.py <snap.json> [...]

Per dump:
* a conversation (no board-3 dialog open) also runs A1's conversation checks
  (`tools/a1/ux_checks.py`: column / composer widths and gutters, clipped
  text, hit targets, row overlap);
* the extras above the composer (queued chip, recovery notice, seat line,
  read-only peer row) sit inside the composer's column, above it and below
  the session strip, with their text inside them and their controls >= 28 px;
* a code block's banner (language, Copy) and highlighted body sit inside the
  block; the block inside the prose column; Copy >= 28 px;
* a transcript notice's title and body sit inside the notice row;
* an open board-3 dialog lies inside the module, its texts inside the card,
  its buttons >= 28 px.

Prints PASS/FAIL lines and one SUMMARY line per dump (the CSV `checks` cell).
Exit 0 only when every check of every dump passed.
"""
import importlib.util
import json
import os
import re
import sys

TOL = 1.5
HERE = os.path.dirname(os.path.abspath(__file__))


def load_a1():
    path = os.path.join(HERE, "..", "a1", "ux_checks.py")
    spec = importlib.util.spec_from_file_location("a1_ux_checks", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


A1 = load_a1()


def rows_of(path):
    return A1.load(path)


def find(rows, wid):
    return A1.find(rows, wid)


def all_of(rows, pred):
    return [w for w in rows if pred(w["i"])]


def inside(a, b, tol=TOL):
    return A1.inside(a, b, tol)


def right(r):
    return r[0] + r[2]


def bottom(r):
    return r[1] + r[3]


def big(r):
    return r[2] >= 28 - 0.5 and r[3] >= 28 - 0.5


def dialog_checks(rows, rep):
    module = next((w for w in rows if w["ty"] == "MpModuleView"), None)
    card = find(rows, "b3_dialog") or find(rows, "b3_root")
    mod_r = module["r"] if module else None
    rep.check("dialog inside the module", mod_r is None or inside(card["r"], mod_r),
              f"card {card['r']} module {mod_r}", "card inside")
    # The dialog's own text (the transcript and strip behind the scrim also
    # carry `b3_` rows: the notices `b3_tl_*`, the strip `b3_strip_*`).
    own = re.compile(r"b3_(ck|img|switch|close|title|sub|dialog)")
    texts = [w for w in rows if own.match(w["i"]) and w["ty"] in ("Label", "TextInput")]
    out = [w["i"] for w in texts if not inside(w["r"], card["r"])]
    rep.check("dialog text inside the card", not out, f"{len(texts)} texts; out: {out[:6]}",
              f"text {len(texts) - len(out)}/{len(texts)} inside")
    buttons = [w for w in rows if w["ty"] == "Button" and w["i"].startswith("b3_")
               and inside(w["r"], card["r"]) and not w["i"].endswith("_x")]
    small = [(w["i"], w["r"][2], w["r"][3]) for w in buttons if not big(w["r"])]
    rep.check("dialog controls >= 28 px", not small, f"{len(buttons)} buttons; small: {small}",
              f"{len(buttons)} controls>=28")


def extras_checks(rows, rep):
    comp = find(rows, "i0_composer")
    strip = find(rows, "b3_strip_root")
    extras = [w for w in rows if w["i"] in ("queue_chip", "recovery_notice", "seat_status", "peer_readonly")]
    if not extras:
        return
    anchor = comp or find(rows, "composer_row")
    for x in extras:
        r = x["r"]
        if anchor:
            a = anchor["r"]
            rep.check(f"{x['i']} in the composer column", r[0] >= a[0] - TOL and right(r) <= right(a) + TOL,
                      f"{r} vs composer {a}", f"{x['i']} in column")
            if comp:
                rep.check(f"{x['i']} above the composer", bottom(r) <= a[1] + TOL,
                          f"bottom {bottom(r):.0f} <= composer top {a[1]:.0f}")
        if strip:
            rep.check(f"{x['i']} below the session strip", r[1] >= bottom(strip["r"]) - TOL,
                      f"top {r[1]:.0f} >= strip bottom {bottom(strip['r']):.0f}")
        prefix = {"queue_chip": "queue_", "recovery_notice": "recovery_", "seat_status": "seat_status",
                  "peer_readonly": "peer_readonly"}[x["i"]]
        texts = [w for w in rows if w["i"].startswith(prefix) and w["ty"] == "Label"]
        out = [w["i"] for w in texts if not inside(w["r"], r)]
        rep.check(f"{x['i']} text inside", not out, f"{len(texts)} labels; out: {out}",
                  f"{x['i']} text {len(texts) - len(out)}/{len(texts)}")
        hits = [w for w in rows if w["ty"] == "Button" and w["i"].startswith(prefix)]
        small = [(w["i"], w["r"][2], w["r"][3]) for w in hits if not big(w["r"])]
        if hits:
            rep.check(f"{x['i']} controls >= 28 px", not small, f"{[(w['i'], w['r']) for w in hits]}",
                      f"{len(hits)} {x['i']} controls>=28")


def code_checks(rows, rep):
    blocks = [w for w in rows if re.fullmatch(r"i\d+_code_\d+", w["i"])]
    pane = find(rows, "conversation_column")
    lst = find(rows, "timeline_list")
    for b in blocks:
        r = b["r"]
        # A block cut by the scrolled list's edge is a viewport clip.
        if lst and (r[1] <= lst["r"][1] + 1 or bottom(r) >= bottom(lst["r"]) - 1):
            continue
        parts = [w for w in rows if w["i"].startswith(b["i"] + "_") and w["ty"] in ("Label", "A7CodeLines", "Markdown")]
        out = [w["i"] for w in parts if not inside(w["r"], r)]
        rep.check(f"{b['i']} banner + body inside", not out, f"{len(parts)} parts; out: {out}",
                  f"{b['i']} {len(parts) - len(out)}/{len(parts)} inside")
        if pane:
            rep.check(f"{b['i']} inside the pane", inside(r, pane["r"]), f"{r} in {pane['r']}")
    copies = [w for w in rows if re.fullmatch(r"code_copy_\d+", w["i"])]
    small = [(w["i"], w["r"]) for w in copies if not big(w["r"])]
    if copies:
        rep.check("Copy controls >= 28 px", not small, f"{[(w['i'], w['r']) for w in copies]}",
                  f"{len(copies)} Copy>=28")


def notice_checks(rows, rep):
    for n in [w for w in rows if w["i"] == "b3_tl_notice"]:
        parts = [w for w in rows if re.fullmatch(r"b3_tl_notice_(title|body)_\d+", w["i"])
                 and n["r"][1] - TOL <= w["r"][1] <= bottom(n["r"]) + TOL]
        out = [w["i"] for w in parts if not inside(w["r"], n["r"])]
        rep.check("notice text inside its row", not out, f"{len(parts)} labels; out: {out}",
                  f"notice text {len(parts) - len(out)}/{len(parts)}")


def check_dump(path):
    rows = rows_of(path)
    rep = A1.Report()
    print(f"== {os.path.basename(path)}")
    if find(rows, "b3_dialog") or find(rows, "b3_root"):
        dialog_checks(rows, rep)
    elif find(rows, "conversation_column"):
        A1.conversation(rows, rep)
        extras_checks(rows, rep)
        code_checks(rows, rep)
        notice_checks(rows, rep)
    return rep.done()


def main():
    status = 0
    for path in sys.argv[1:]:
        status |= check_dump(path)
    sys.exit(status)


if __name__ == "__main__":
    main()
