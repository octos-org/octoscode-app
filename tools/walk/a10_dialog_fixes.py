#!/usr/bin/env python3
"""A10 — the judged defects in A5's dialogs, measured by click walk:

1. Models: the "Test route" / "Discover models" pills sat flush on the
   provider card's bottom edge (atlas card y 110 h 290 ends at 400; the pills
   y 356 h 44 end at 400 too), so the card border cut their bottoms.
   Check: each pill lies inside the provider card with a bottom inset equal
   to its side inset (16 px design), desktop and phone.
2. Loops: the row icons were 33 / 32 / 48 px (a 1.6-unit stroke drawn
   2.0 / 1.9 / 2.9 px wide). Check: every row icon is one size (22 px design),
   on one centre line per row, at one pitch; the Monitors dialog's icons are
   the same size.

usage: OCTOSCODE_APP_BIN=<host octosense> a10_dialog_fixes.py <desktop|phone> <outdir>
`run_session` starts the app hidden with OCTOSCODE_SYNTHETIC_LIVE=1
OCTOSCODE_DIALOG_SEED=1 (the A5 reference-board fixture; no server) and its
state isolated in a per-run temp dir (brief §8), then ALWAYS stops it; every
dialog is opened by a palette CLICK.
"""
import sys

from a10_lib import Walk, checks_line, dialog_checks, run_session

MODE = sys.argv[1] if len(sys.argv) > 1 else "desktop"
OUT = sys.argv[2] if len(sys.argv) > 2 else f"docs/ux/a10/fixes/{MODE}"
W: Walk = None  # set by walk()


def close_dialog():
    if W.visible("dialog_close"):
        W.click("dialog_close")
        W.wait_shown("dialog_frame", 5, gone=True)


def models():
    W.note("== Models: the route pills sit inside the provider card")
    W.mark()
    W.check("models: /model palette row CLICK opens the dialog",
            W.palette_run("mo", "/model") and W.wait_shown("dlg_models_card_deepseek", 8))
    sn = W.snap()
    card = W.rect("dlg_models_card_deepseek", sn=sn)
    for pill in ("dlg_models_btn_test_surface", "dlg_models_btn_discover_surface"):
        r = W.rect(pill, sn=sn)
        if not (card and r):
            W.check(f"models: {pill} drawn", False, f"card={card} pill={r}")
            continue
        side = r[0] - card[0] if pill.endswith("test_surface") else (card[0] + card[2]) - (r[0] + r[2])
        bottom = (card[1] + card[3]) - (r[1] + r[3])
        W.check(f"models: {pill} inside the card with a bottom inset >= 8 and ~= its side inset",
                bottom >= 8 and abs(bottom - side) <= 2.5,
                f"pill {r} card {card} side={side:.1f} bottom={bottom:.1f}")
    c = dialog_checks(sn, "dialog_frame", "dlg_models_")
    W.check("models: dialog numeric checks", c["ok"], checks_line(c))
    W.shot(f"models-{MODE}")
    close_dialog()


def loops():
    W.note("== Loops: one icon size, one centre line, one pitch")
    W.check("loops: /loop palette row CLICK opens the dialog",
            W.palette_run("loo", "/loop") and W.wait_shown("dlg_loops_loops_card", 8))
    sn = W.snap()
    sizes = set()
    for i in (1, 2, 3):
        icons = [(k, W.rect(f"dlg_loops_loop_{i}_{k}", sn=sn)) for k in ("pause", "play", "trash")]
        icons = [(k, r) for k, r in icons if r]
        if not icons:
            continue
        for _, r in icons:
            sizes.add((round(r[2]), round(r[3])))
        mids = [r[1] + r[3] / 2 for _, r in icons]
        W.check(f"loops: row {i} icons share one centre line",
                max(mids) - min(mids) <= 1.0, f"{[(k, r) for k, r in icons]}")
        xs = [r[0] + r[2] / 2 for _, r in icons]
        gaps = [round(b - a, 1) for a, b in zip(xs, xs[1:])]
        W.check(f"loops: row {i} icons at one pitch (±1 px scale rounding)", max(gaps, default=0) - min(gaps, default=0) <= 1.0, f"pitches {gaps}")
        card = W.rect("dlg_loops_loops_card", sn=sn)
        last = icons[-1][1]
        W.check(f"loops: row {i} last icon inside the card",
                card and last[0] + last[2] <= card[0] + card[2] - 8, f"icon {last} card {card}")
    W.check("loops: every row icon is one square size <= 24 px",
            len(sizes) == 1 and all(w == h and w <= 24 for w, h in sizes), f"sizes {sorted(sizes)}")
    hits = [w for w in W.prefixed("dlg_loops_loop_", sn) if w["i"].endswith("_hit")]
    W.check("loops: every icon hit target >= 28 px", hits and all(w["r"][2] >= 28 and w["r"][3] >= 28 for w in hits),
            f"{len(hits)} hits, min {min((min(w['r'][2], w['r'][3]) for w in hits), default=0)}")
    c = dialog_checks(sn, "dialog_frame", "dlg_loops_")
    W.check("loops: dialog numeric checks", c["ok"], checks_line(c))
    W.shot(f"loops-{MODE}")
    close_dialog()
    W.check("monitors: /monitor palette row CLICK opens the dialog",
            W.palette_run("moni", "/monitor") and W.wait_shown("dlg_monitors_mon_1_pause", 8))
    sn = W.snap()
    msizes = {(round(r[2]), round(r[3])) for r in
              (W.rect(f"dlg_monitors_mon_{i}_{k}", sn=sn) for i in (1, 2) for k in ("pause", "trash")) if r}
    W.check("monitors: row icons are the Loops dialog's size", msizes == sizes, f"monitors {sorted(msizes)} loops {sorted(sizes)}")
    c = dialog_checks(sn, "dialog_frame", "dlg_monitors_")
    W.check("monitors: dialog numeric checks", c["ok"], checks_line(c))
    W.shot(f"monitors-{MODE}")
    close_dialog()


def walk(w: Walk) -> None:
    global W
    W = w
    models()
    loops()


if __name__ == "__main__":
    sys.exit(run_session(walk, mode=MODE, outdir=OUT, replay_port=None,
                         env={"OCTOSCODE_SYNTHETIC_LIVE": "1", "OCTOSCODE_DIALOG_SEED": "1"}))
